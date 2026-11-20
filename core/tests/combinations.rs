//! 组合场景测试：多个特性同时开启时是否互相干扰。
//!
//! 之前每个特性都单独测过，但它们组合起来没测过。

use std::sync::Arc;

use archtype_core::convert::Converter;
use archtype_core::lm::BigramLm;
use archtype_core::{Engine, Lexicon, MemoryLexicon};

fn engine(lexicon_tsv: &str) -> Engine {
    let lexicon: Arc<dyn Lexicon> = Arc::new(MemoryLexicon::from_tsv_str(lexicon_tsv).unwrap());
    Engine::new(lexicon, Arc::new(BigramLm::default()))
}

#[test]
fn shuangpin_with_fuzzy() {
    // 双拼输入 zidk = zi + dao，词库里只有 zhi dao，需要模糊音兜底
    let mut e = engine("zhi dao\t知道\t5000\n");
    e.set_shuangpin(true);
    e.set_fuzzy(true);
    let list = e.input("zidk");
    assert!(
        list.iter().any(|c| c.text == "知道"),
        "双拼 + 模糊音应当命中 知道: {:?}",
        list.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
}

#[test]
fn fuzzy_adds_without_replacing_typed() {
    // 词库同时有照着打的 zi dao 和模糊音的 zhi dao：模糊音只能加候选，不能把照着打的顶掉
    let mut e = engine("zi dao\t自道\t5000\nzhi dao\t知道\t5000\n");
    e.set_shuangpin(true);
    e.set_fuzzy(true);
    let list = e.input("zidk");
    let texts: Vec<&str> = list.iter().map(|c| c.text.as_str()).collect();
    let typed = texts.iter().position(|t| *t == "自道");
    let fuzzy = texts.iter().position(|t| *t == "知道");
    assert!(typed.is_some(), "照着打的候选不见了: {texts:?}");
    if let (Some(typed), Some(fuzzy)) = (typed, fuzzy) {
        assert!(typed < fuzzy, "模糊音候选排在照着打的候选前面: {texts:?}");
    }
}

#[test]
fn traditional_with_user_word() {
    let mut e = engine("ni hao\t你好\t5000\n");
    e.install_converter(Converter::from_tsv_str("国\t國\n").unwrap());
    e.set_traditional(true);
    e.add_user_word("zhong guo", "中国", 1000.0).unwrap();
    let list = e.input("zhongguo");
    assert!(
        list.iter().any(|c| c.text == "中國"),
        "用户词也要跟着转繁体: {:?}",
        list.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
}

#[test]
fn traditional_with_context() {
    let mut e = engine("xian\t先\t50000\nxi an\t西安\t1000\n");
    // 真的把字换掉（不是恒等映射），才验证得到「输出被转换」；上下文仍应选中 xi an 的西安
    e.install_converter(Converter::from_tsv_str("安\t晏\n").unwrap());
    e.set_traditional(true);
    e.set_context("我");
    let list = e.input("xian");
    assert!(
        list.iter().any(|c| c.text == "西晏"),
        "上下文生效时 xi an 的整词应当被转换: {:?}",
        list.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
}

#[test]
fn shuangpin_keeps_raw_candidate() {
    let mut e = engine("ni hao\t你好\t5000\n");
    e.set_shuangpin(true);
    // 双拼模式下打英文，原始串要原样给出，而不是被当成双拼键位
    let list = e.input("hello");
    assert!(
        list.iter().any(|c| c.text == "hello"),
        "英文串应当保留: {:?}",
        list.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
}

#[test]
fn repeated_selection_boosts_candidate() {
    // 两个同拼音候选，频次相差不大；连续选低频的那个应当把它顶上来
    let e = engine("shi\t是\t1000\nshi\t事\t900\n");
    assert_eq!(e.input("shi")[0].text, "是");
    for _ in 0..6 {
        e.record_selection("事");
    }
    let list = e.input("shi");
    assert_eq!(
        list[0].text,
        "事",
        "多次选用后应当排到前面: {:?}",
        list.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
}
#[test]
fn auto_learn_under_shuangpin() {
    // 插件在双拼模式下会把「键位串」当拼音传进来，自动学习要把它们展开成全拼
    let mut e = engine("ni hao\t你好\t5000\n");
    e.set_shuangpin(true);
    // 阿 = aa，特 = te
    for _ in 0..3 {
        e.record_selection_with_pinyin("阿特", Some("aate"));
    }
    assert!(
        e.user_words().iter().any(|(_, word, _)| word == "阿特"),
        "双拼下的选词应当能进用户词库: {:?}",
        e.user_words()
    );
    let list = e.input("aate");
    assert!(
        list.iter().any(|c| c.text == "阿特"),
        "学到的词应当能用双拼打出来: {:?}",
        list.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
}

#[test]
fn shuangpin_with_traditional() {
    let mut e = engine("zhong guo\t中国\t8000\n");
    e.set_shuangpin(true);
    e.set_traditional(true);
    e.install_converter(Converter::from_tsv_str("国\t國\n").unwrap());
    let list = e.input("vsgo");
    assert!(
        list.iter().any(|c| c.text == "中國"),
        "双拼 + 繁体: {:?}",
        list.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
}

#[test]
fn fuzzy_with_traditional() {
    let mut e = engine("zhong guo\t中国\t8000\n");
    e.set_fuzzy(true);
    e.set_traditional(true);
    e.install_converter(Converter::from_tsv_str("国\t國\n").unwrap());
    // zongguo 靠模糊音匹配 zhong guo，输出再转繁体
    let list = e.input("zongguo");
    assert!(
        list.iter().any(|c| c.text == "中國"),
        "模糊音 + 繁体: {:?}",
        list.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
}
