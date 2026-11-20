//! 麦克风采集（cpal），跨平台：Linux 走 ALSA/PulseAudio，macOS 走 CoreAudio。
//!
//! 回调里直接把设备格式转成 16kHz 单声道 i16 再吐出去，上层不用关心采样格式。

use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::audio::{downmix_into, to_i16_into, Resampler, TARGET_RATE};

/// 采集信息（用于日志和调试）。
#[derive(Clone, Debug)]
pub struct CaptureInfo {
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub format: String,
}

/// 持有 stream 的句柄；丢弃即停止采集。
pub struct Capture {
    _stream: cpal::Stream,
    pub info: CaptureInfo,
}

/// 列出可用的录音设备名。
pub fn list_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let mut names = Vec::new();
    for dev in host.input_devices().context("枚举录音设备失败")? {
        if let Ok(name) = dev.name() {
            names.push(name);
        }
    }
    Ok(names)
}

/// 候选录音设备，按优先级：指定名字的那个 > 系统默认 > 其余。
fn candidate_devices(host: &cpal::Host, want: Option<&str>) -> Result<Vec<cpal::Device>> {
    let mut all: Vec<cpal::Device> = host
        .input_devices()
        .context("枚举录音设备失败")?
        .collect();
    let mut ordered: Vec<cpal::Device> = Vec::new();
    if let Some(want) = want {
        if let Some(pos) = all
            .iter()
            .position(|d| d.name().ok().as_deref() == Some(want))
        {
            ordered.push(all.remove(pos));
        } else {
            eprintln!("找不到设备 {want:?}，改用默认设备");
        }
    }
    if let Some(dev) = host.default_input_device() {
        let name = dev.name().unwrap_or_default();
        match all.iter().position(|d| d.name().ok() == Some(name.clone())) {
            Some(pos) => ordered.push(all.remove(pos)),
            None => ordered.push(dev),
        }
    }
    ordered.extend(all);
    Ok(ordered)
}

/// 用这个设备建一条流。建不起来就把错误带回去，让调用方换下一个设备。
///
/// Linux 上 cpal 走 ALSA，「默认设备」是 ALSA 的 default（PipeWire 的插件）：它依赖
/// PipeWire 有默认录音源，没设好时查参数能过、真正开流才报 snd_pcm_hw_params 失败。
fn open_stream<F>(dev: &cpal::Device, sink: Arc<Mutex<F>>) -> Result<Capture>
where
    F: FnMut(Vec<i16>) + Send + 'static,
{
    // 设备能直接给 16kHz 就要 16kHz（PipeWire/PulseAudio 自己重采样，质量比我们好），
    // 声道越少越好；不行再用设备默认格式，自己滤波重采样
    let wanted = cpal::SampleRate(TARGET_RATE);
    let preferred = dev.supported_input_configs().ok().and_then(|configs| {
        configs
            .filter(|c| {
                c.min_sample_rate() <= wanted
                    && wanted <= c.max_sample_rate()
                    && matches!(
                        c.sample_format(),
                        cpal::SampleFormat::F32 | cpal::SampleFormat::I16
                    )
            })
            .min_by_key(|c| (c.channels(), c.sample_format() != cpal::SampleFormat::F32))
            .map(|c| c.with_sample_rate(wanted))
    });
    let supported = match preferred {
        Some(config) => config,
        None => dev.default_input_config().context("读取录音配置失败")?,
    };
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let channels = config.channels;
    let rate = config.sample_rate.0;
    let info = CaptureInfo {
        device: dev.name().unwrap_or_default(),
        sample_rate: rate,
        channels,
        format: format!("{format:?}"),
    };

    // 三个分支都要用同一个回调，用 Arc<Mutex> 共享；每次回调只会在一个线程里跑。
    let err_fn = |e| eprintln!("录音流错误: {e}");

    let stream = match format {
        cpal::SampleFormat::F32 => {
            let mut rs = Resampler::new(rate, TARGET_RATE);
            let sink = sink.clone();
            // 回调里复用这几块缓冲：实时音频线程上每帧三次堆分配会拖慢处理
            let mut mono = Vec::new();
            let mut res = Vec::new();
            let mut pcm = Vec::new();
            dev.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    downmix_into(data, channels, &mut mono);
                    rs.process_into(&mono, &mut res);
                    to_i16_into(&res, &mut pcm);
                    if let Ok(mut f) = sink.lock() {
                        f(std::mem::take(&mut pcm));
                    }
                },
                err_fn,
                None,
            )?
        }
        cpal::SampleFormat::I16 => {
            let mut rs = Resampler::new(rate, TARGET_RATE);
            let sink = sink.clone();
            let mut f32s = Vec::new();
            let mut mono = Vec::new();
            let mut res = Vec::new();
            let mut pcm = Vec::new();
            dev.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    f32s.clear();
                    f32s.extend(data.iter().map(|&v| v as f32 / 32768.0));
                    downmix_into(&f32s, channels, &mut mono);
                    rs.process_into(&mono, &mut res);
                    to_i16_into(&res, &mut pcm);
                    if let Ok(mut f) = sink.lock() {
                        f(std::mem::take(&mut pcm));
                    }
                },
                err_fn,
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let mut rs = Resampler::new(rate, TARGET_RATE);
            let sink = sink.clone();
            let mut f32s = Vec::new();
            let mut mono = Vec::new();
            let mut res = Vec::new();
            let mut pcm = Vec::new();
            dev.build_input_stream(
                &config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    f32s.clear();
                    f32s.extend(data.iter().map(|&v| (v as f32 - 32768.0) / 32768.0));
                    downmix_into(&f32s, channels, &mut mono);
                    rs.process_into(&mono, &mut res);
                    to_i16_into(&res, &mut pcm);
                    if let Ok(mut f) = sink.lock() {
                        f(std::mem::take(&mut pcm));
                    }
                },
                err_fn,
                None,
            )?
        }
        other => return Err(anyhow!("不支持的采样格式: {other:?}")),
    };

    stream.play().context("启动录音流失败")?;
    Ok(Capture {
        _stream: stream,
        info,
    })
}

/// 开始采集；回调收到的是 16kHz 单声道 i16。
pub fn start<F>(device: Option<&str>, on_pcm: F) -> Result<Capture>
where
    F: FnMut(Vec<i16>) + Send + 'static,
{
    let host = cpal::default_host();
    let sink = Arc::new(Mutex::new(on_pcm));
    let candidates = candidate_devices(&host, device)?;
    let mut errors: Vec<String> = Vec::new();
    for dev in &candidates {
        match open_stream(dev, sink.clone()) {
            Ok(capture) => {
                if !errors.is_empty() {
                    eprintln!(
                        "录音设备 {:?} 可用，前面试过的打不开：{}",
                        capture.info.device,
                        errors.join("；")
                    );
                }
                return Ok(capture);
            }
            Err(error) => {
                let name = dev.name().unwrap_or_default();
                errors.push(format!("{name}: {error:#}"));
            }
        }
    }
    if errors.is_empty() {
        return Err(anyhow!("没有可用的录音设备"));
    }
    Err(anyhow!("没有能打开的录音设备（{}）", errors.join("；")))
}
