//! 最小 WAV 读写：16kHz 单声道 16bit。
// MSRV 1.82：`as_chunks` 要 1.88。
#![allow(clippy::chunks_exact_to_as_chunks)]

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};

pub const SAMPLE_RATE: u32 = 16_000;

/// 写 16kHz 单声道 16bit WAV。
pub fn write_wav<P: AsRef<Path>>(path: P, samples: &[i16]) -> Result<()> {
    // RIFF 的长度字段是 32 位：样本太多时直接报错，别让 `as u32` 静默截断
    let data_len = u32::try_from(samples.len().saturating_mul(2)).context("WAV 太大")?;
    let riff_len = data_len.checked_add(36).context("WAV 太大")?;
    let mut f = File::create(path.as_ref())
        .with_context(|| format!("创建 WAV 失败: {}", path.as_ref().display()))?;
    f.write_all(b"RIFF")?;
    f.write_all(&riff_len.to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&SAMPLE_RATE.to_le_bytes())?;
    f.write_all(&(SAMPLE_RATE * 2).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for s in samples {
        f.write_all(&s.to_le_bytes())?;
    }
    Ok(())
}

/// 读 16bit PCM WAV，返回 (sample_rate, channels, samples)。
pub fn read_wav<P: AsRef<Path>>(path: P) -> Result<(u32, u16, Vec<i16>)> {
    let mut buf = Vec::new();
    File::open(path.as_ref())
        .with_context(|| format!("打开 WAV 失败: {}", path.as_ref().display()))?
        .read_to_end(&mut buf)?;
    if buf.len() < 12 || &buf[0..4] != b"RIFF" || &buf[8..12] != b"WAVE" {
        bail!("不是 WAV 文件");
    }
    let mut pos = 12;
    let mut rate = 0u32;
    let mut channels = 1u16;
    let mut has_fmt = false;
    while pos + 8 <= buf.len() {
        let id = &buf[pos..pos + 4];
        let size = u32::from_le_bytes(buf[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = pos + 8;
        if id == b"fmt " && body + 16 <= buf.len() {
            channels = u16::from_le_bytes(buf[body + 2..body + 4].try_into().unwrap());
            rate = u32::from_le_bytes(buf[body + 4..body + 8].try_into().unwrap());
            has_fmt = true;
        } else if id == b"data" {
            if !has_fmt || rate == 0 || channels == 0 {
                bail!("WAV 的 fmt 段缺失或非法（采样率/声道为 0）");
            }
            let end = (body + size).min(buf.len());
            let samples = buf[body..end]
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
            return Ok((rate, channels, samples));
        }
        pos = body + size + (size & 1);
    }
    bail!("WAV 里没有 data 段")
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let path = std::env::temp_dir().join("archtype_wav_roundtrip.wav");
        let samples: Vec<i16> = (0..1000).map(|i| (i % 100) as i16 - 50).collect();
        write_wav(&path, &samples).unwrap();
        let (rate, channels, back) = read_wav(&path).unwrap();
        assert_eq!(rate, SAMPLE_RATE);
        assert_eq!(channels, 1);
        assert_eq!(back, samples);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_non_wav() {
        let path = std::env::temp_dir().join("archtype_not_wav.bin");
        std::fs::write(&path, b"not a wav file at all").unwrap();
        assert!(read_wav(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }
}
