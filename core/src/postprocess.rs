//! 候选的稳定化后处理。
//!
//! 束搜索给出的候选按分数排，但「打全了的词一定要在补全前面」「纠错只加候选、不替掉
//! 照着打的读法」这类规则不是分数能表达的，只能按位置挪。这里集中这些步骤：
//! 每一步只做一种稳定化，[`STEPS`] 的顺序就是语义顺序，[`stabilize_input`] 按它依次执行。
//!
//! 为什么不留在 `lib.rs` 里：这条链是改动最频繁、判据最琐碎的地方（每个函数都绑着
//! 回归集里的具体输入），和引擎主流程放在同一个文件里时，改一条规则要在 6000 行里翻。
//! 排序不变量与回归基线见 `docs/ENGINE-INVARIANTS.md` 与 `tools/eval/`。
//!
//! 顺序为什么不能随便换：
//!
//! - [`protect_exact_input`] 先把打全的实词提到最前，后面三个「排在谁后面」的步骤
//!   才有稳定的锚点；
//! - [`typos_after_typed`] 按位置再拦一次纠错词——前面若有人挪动过候选，
//!   束搜索阶段的分数压制会被绕过去（`sheihaiyo` 的注释里有完整例子）；
//! - [`promote_picked`] 在最后改首选，[`pin_single_initial`] 再对单声母输入做一次纠正，
//!   两者都依赖前面已经排好的顺序。
//!
//! 每一步都不增删候选，只重排与压分；[`stabilize_input`] 因此可以安全地整段调用。

use crate::learn;
use crate::{is_raw_candidate, Candidate, CandidateWord};
use crate::{EXACT_LONG_MIN_FREQ, EXACT_LONG_SYLLABLES, EXACT_PROTECT_MIN_FREQ};

/// 后处理的一步。`input` 是规范化后的输入（小写、去掉首尾空白），只有
/// [`pin_single_initial`] 用得到；其余步骤忽略它，签名统一是为了让 [`STEPS`] 成为一条
/// 可以按顺序遍历的链，而不是散落在调用点的六个函数调用。
pub(crate) type Step = (&'static str, fn(&mut Vec<Candidate>, &str));

/// 顺序即语义。名字同时用于失败信息与测试，调整顺序时这里和函数文档要一起改。
pub(crate) const STEPS: &[Step] = &[
    ("打全的实词提前", protect_exact_input),
    ("补全让位于打全的词", completions_after_exact_words),
    ("纠错让位于照着打的读法", typos_after_typed),
    ("模糊音让位于精确词", fuzzy_after_exact_word),
    ("选过的词前移", promote_picked),
    ("单声母字保底", pin_single_initial),
];

/// 依次执行 [`STEPS`]。
pub(crate) fn stabilize_input(candidates: &mut Vec<Candidate>, input: &str) {
    for (_, step) in STEPS {
        step(candidates, input);
    }
}

/// 输入正好是一个完整音节（xian、ji、zhe）时：本音节的字排在补全出来的字（想、就、真）前面，
/// 纠错出来的（gei → 被、飞；si → 是，漏按 h）再往后。
///
/// 补全每个字母只罚 1.4 左右，高频字补全以后常常比本音节的字分还高；已经打全的音节，
/// 用户要的是这个音节的字。多音节输入、没打全的音节不动。
pub(crate) fn exact_syllable_first(candidates: Vec<Candidate>) -> Vec<Candidate> {
    let exact = |c: &Candidate| c.features.typo_count == 0.0 && c.features.prefix_completion == 0.0;
    let one_syllable = candidates
        .iter()
        .any(|c| exact(c) && c.words.len() == 1 && c.words[0].known && !c.pinyin.contains(' '));
    if !one_syllable {
        return candidates;
    }
    let (mut out, rest): (Vec<Candidate>, Vec<Candidate>) = candidates.into_iter().partition(exact);
    lift_split_word(&mut out);
    // 补全的在前、纠错的在后，各自按分数
    let (completions, typos): (Vec<Candidate>, Vec<Candidate>) =
        rest.into_iter().partition(|c| c.features.typo_count == 0.0);
    // 分数也压到前面的下面，后面按分数合并、排序的地方不会再把它们换上来
    for c in completions.into_iter().chain(typos) {
        let floor = out.last().map_or(0.0, |last| last.score);
        out.push(Candidate {
            score: c.score.min(floor - 1e-3),
            ..c
        });
    }
    out
}

/// 一个音节也能切成两个音节的整词（xian → 西安）：分数和第一名差得不多时，至少排到第 [`SPLIT_WORD_SLOT`] 位。
/// 单字太多，按分数它常常排在十几位以后，第一页看不到。
fn lift_split_word(exact: &mut Vec<Candidate>) {
    let Some(top) = exact.first().map(|c| c.score) else {
        return;
    };
    let found = exact.iter().position(|c| {
        c.words.len() == 1 && c.pinyin.contains(' ') && top - c.score <= SPLIT_WORD_MARGIN
    });
    let Some(at) = found.filter(|&at| at > SPLIT_WORD_SLOT) else {
        return;
    };
    let mut word = exact.remove(at);
    // 分数夹在前后两个之间，后面再按分数排也不会变
    let (above, below) = (
        exact[SPLIT_WORD_SLOT - 1].score,
        exact[SPLIT_WORD_SLOT].score,
    );
    word.score = (above + below) / 2.0;
    exact.insert(SPLIT_WORD_SLOT, word);
}

/// [`lift_split_word`]：最多和第一名差多少（自然对数）。实测 xian 的「西安」差 3.0，
/// 别的音节能切出来的整词（提案、彼岸、立案、堤岸）差 4 以上，不抬。
const SPLIT_WORD_MARGIN: f64 = 3.5;
/// [`lift_split_word`]：抬到第几位（从 0 数，4 是第五个）。
const SPLIT_WORD_SLOT: usize = 4;

/// 拼音打全了、而且正好是一个实在的词时，这个词必须排第一。
///
/// 补全和纠错的分数里带着词频，高频词补出来以后可能比用户打全了的低频词还高：
/// `zhongdi` 会出「重点」（zhong dian）而不是「种地」，`ka` 出「看」，`beiji` 出「北京」。
/// 用户按空格上屏的是第一个——打到一半时补全排第二只是多按一个键，
/// 打全了却被换成别的词就是打错字。所以排在这个词前面的补全/纠错候选都挪到它后面。
///
/// 同理，整个输入就是一个词时，几个单字硬凑出来的候选（`shidan` 拆成「是 | 但」）
/// 也不该压过它（「实弹」）。
///
/// 「实在的词」：精确匹配（不靠纠错和补全）、要么整个输入是一个词，要么最后一个词
/// 至少两个字（这时只和「前面的词一样、只有最后一个词不同」的候选比）；每个词的词频不低于 [`EXACT_PROTECT_MIN_FREQ`]，或者是用户选过的。
/// 最后是个单字的精确结果（`zhonggu` 拆成「中 | 骨」）不算，用户多半还没打完，
/// 补全照常可以排第一。
fn protect_exact_input(candidates: &mut Vec<Candidate>, _input: &str) {
    let multi = |w: &CandidateWord| w.pinyin.split_whitespace().count() >= 2;
    // 模糊音兜底的词不是照着打的（jiesu 的「结束」是 jie shu）：当了锚点的话下面一个都不挪，
    // 补全出来的「结算」就一直压在打全了的「借宿」前面
    let is_exact = |c: &Candidate| {
        c.features.coverage_score >= 1.0
            && c.features.typo_count == 0.0
            && c.features.prefix_completion == 0.0
            && !c.features.fuzzy
            && !c.words.is_empty()
            && c.words.iter().all(|w| w.known)
    };
    // 三个音节以上、整个输入正好是词库里的一个词：读音这么长还能对上，同音的词库词平均不到两个，
    // 不看词频也算实在的词（前后端、启动器、候选词在 rime-ice 里都只有占位的词频）
    // 切法得是最自然的那种：有个音节更少、够常用的精确整词时（jieran 的「截然」jie ran），
    // 换一种切法才对得上的长词（己二胺 ji er an）不算
    let fewest = fewest_exact_syllables(candidates);
    let long_word = |c: &Candidate| {
        let syllables = c.pinyin.split_whitespace().count();
        c.words.len() == 1
            && c.words[0].freq >= EXACT_LONG_MIN_FREQ
            && syllables >= EXACT_LONG_SYLLABLES
            && fewest.is_none_or(|n| syllables <= n)
    };
    let solid = |c: &Candidate| {
        is_exact(c)
            && (c.words.len() == 1 || c.words.last().map(multi).unwrap_or(false))
            && (c.features.history_count > 0.0
                || long_word(c)
                || c.words.iter().all(|w| w.freq >= EXACT_PROTECT_MIN_FREQ))
    };
    // 长的整词先提到拼出来的整句前面：fayiban 里 法 | 一般 本身也算「实在的」（最后一个词两个字），
    // 下面按第一个实在的候选当锚点时轮不到「发一版」。用户选过的整句不动
    // 只压过读音完全一样的拼凑（启动 | 其 = qi dong qi）；分数只抬到刚好高过它，后面的不动
    // （整段压成等差会让 compose_candidates 按分差去掉硬凑整句的判断失效）
    if let Some(word_at) = candidates.iter().position(|c| is_exact(c) && long_word(c)) {
        let stitched = candidates[..word_at].iter().position(|c| {
            c.words.len() >= 2
                && c.features.history_use_count == 0.0
                && same_syllables(c, &candidates[word_at])
        });
        if let Some(at) = stitched {
            let mut word = candidates.remove(word_at);
            word.score = word.score.max(candidates[at].score + 1e-3);
            candidates.insert(at, word);
            for i in 1..candidates.len() {
                if candidates[i].score > candidates[i - 1].score {
                    candidates[i].score = candidates[i - 1].score - 1e-3;
                }
            }
        }
    }
    let Some(anchor) = candidates.iter().position(solid) else {
        return;
    };
    if anchor == 0 {
        return;
    }
    let whole_word = candidates[anchor].words.len() == 1;
    let long_anchor = long_word(&candidates[anchor]);
    let anchor_pinyin = candidates[anchor].pinyin.clone();
    // 精确结果是几个词拼的（我在 | 种地）时，只管开头一样、只是后面换了词的拼凑结果
    // （我 | 在重点）；一个整词盖住全部输入的（从善如流 vs 从善 | 入理）是更强的证据，不动
    let head_text: String = {
        let words = &candidates[anchor].words;
        words[..words.len() - 1]
            .iter()
            .map(|w| w.text.as_str())
            .collect()
    };
    let same_head =
        |c: &Candidate| whole_word || (c.words.len() >= 2 && c.text.starts_with(&head_text));
    let yields = |c: &Candidate| {
        let corrected = c.features.typo_count > 0.0 || c.features.prefix_completion > 0.0;
        // 长的整词前面，几个词拼出来的都让位（启动 | 其、前后 | 断）；短的整词只压过全是单字拼的
        (corrected && same_head(c))
            || (whole_word
                && c.words.len() >= 2
                && ((long_anchor
                    && c.pinyin
                        .split_whitespace()
                        .eq(anchor_pinyin.split_whitespace()))
                    || !c.words.iter().any(multi)))
    };
    if !candidates[..anchor].iter().any(yields) {
        return;
    }
    let mut ahead = Vec::new();
    let mut keep = Vec::with_capacity(candidates.len());
    for (i, c) in candidates.drain(..).enumerate() {
        if i < anchor && yields(&c) {
            ahead.push(c);
        } else {
            keep.push(c);
        }
    }
    let at = keep
        .iter()
        .position(solid)
        .map(|i| i + 1)
        .unwrap_or(keep.len());
    keep.splice(at..at, ahead);
    // 分数改成单调递减，后面再按分数重排（繁体转换）顺序也不变
    for i in 1..keep.len() {
        if keep[i].score >= keep[i - 1].score {
            keep[i].score = keep[i - 1].score - 1e-3;
        }
    }
    *candidates = keep;
}

/// 纠错出来的候选不排在第一个照着打的读法（不纠错、每个词词库里都有、不是简拼、不是模糊音兜底）前面，
/// 除非它用的音节更少（`jintiam` 的「今天」）。用户的原话：纠错应该是「添加可疑的候选词，而不是替代原候选词」。
///
/// 模糊音兜底的词不在这里管：`penyou` 出「朋友」、`shengme` 出「什么」排第一是回归集定死的。
///
/// [`crate::typed_reading_first`] 在束搜索的结果上按分数压过一次，但后面按位置挪候选的步骤会把它绕过去：
/// `sheihaiyo` 里压纠错词「谁还要」用的基准是补全出来的「谁还有」（yo → you），[`protect_exact_input`]
/// 接着把补全的整句挪到「谁嗨哟」后面，「谁还要」就又成了第一个。这里按位置再拦一次，
/// 不管前面谁动过顺序。只挪第一个照着打的读法前面的。
pub(crate) fn typos_after_typed(candidates: &mut Vec<Candidate>, _input: &str) {
    let syllables = |c: &Candidate| c.pinyin.split_whitespace().count();
    let typed = |c: &Candidate| {
        !is_raw_candidate(c)
            && c.features.typo_count == 0.0
            && c.features.full_jianpin == 0.0
            && !c.features.fuzzy
            && !c.words.is_empty()
            && c.words.iter().all(|w| w.known)
    };
    let Some(anchor) = candidates.iter().position(typed) else {
        return;
    };
    let need = syllables(&candidates[anchor]);
    let yields = |c: &Candidate| c.features.typo_count > 0.0 && syllables(c) >= need;
    if !candidates[..anchor].iter().any(yields) {
        return;
    }
    let mut moved = Vec::new();
    let mut keep = Vec::with_capacity(candidates.len());
    for (i, c) in candidates.drain(..).enumerate() {
        if i < anchor && yields(&c) {
            moved.push(c);
        } else {
            keep.push(c);
        }
    }
    let at = keep.iter().position(typed).map_or(keep.len(), |i| i + 1);
    keep.splice(at..at, moved);
    make_scores_monotonic(&mut keep);
    *candidates = keep;
}

/// 照着打、打全了、整个输入正好是词库里一个够常用的词（[`EXACT_PROTECT_MIN_FREQ`]）。
fn is_exact_frequent_word(c: &Candidate) -> bool {
    c.words.len() == 1
        && c.words[0].known
        && c.words[0].freq >= EXACT_PROTECT_MIN_FREQ
        && c.features.coverage_score >= 1.0
        && c.features.typo_count == 0.0
        && c.features.prefix_completion == 0.0
        && c.features.full_jianpin == 0.0
        && !c.features.fuzzy
}

/// 照着打的读法本身就是词库里一个够常用的词时，模糊音兜底的词排在第一个这样的词后面
/// （`penyou`：喷油、朋友；`zuijing`：最精、最近）。
///
/// 照着打只能几个字硬凑（`shengme` 没有 sheng me 这个词）时不动，模糊音的词照旧排第一（什么）：
/// 那时它多半才是用户要的。用户选过的模糊音词也不动。
pub(crate) fn fuzzy_after_exact_word(candidates: &mut Vec<Candidate>, _input: &str) {
    let Some(first) = candidates.iter().position(is_exact_frequent_word) else {
        return;
    };
    let yields = |c: &Candidate| c.features.fuzzy && c.features.history_use_count == 0.0;
    if !candidates[..first].iter().any(yields) {
        return;
    }
    let mut moved = Vec::new();
    let mut keep = Vec::with_capacity(candidates.len());
    for (i, c) in candidates.drain(..).enumerate() {
        if i < first && yields(&c) {
            moved.push(c);
        } else {
            keep.push(c);
        }
    }
    let at = keep
        .iter()
        .position(is_exact_frequent_word)
        .map_or(keep.len(), |i| i + 1);
    keep.splice(at..at, moved);
    make_scores_monotonic(&mut keep);
    *candidates = keep;
}

/// 打全了、整个输入正好是一个够常用的词（[`EXACT_PROTECT_MIN_FREQ`]）时，补全出来的候选都排在这些词后面。
///
/// [`protect_exact_input`] 只认第一个这样的词：它已经排第一时什么都不挪。`fangxiangji` 第一个是「放像机」，
/// 补全的「方向键」（jian）就一直压在同样打全了的「芳香剂」前面。补全只能「加」候选，不能抢打全了的词的位置。
/// 用户选过的补全不挪。
pub(crate) fn completions_after_exact_words(candidates: &mut Vec<Candidate>, _input: &str) {
    let exact_word = is_exact_frequent_word;
    let Some(last) = candidates.iter().rposition(exact_word) else {
        return;
    };
    let yields =
        |c: &Candidate| c.features.prefix_completion > 0.0 && c.features.history_use_count == 0.0;
    if !candidates[..last].iter().any(yields) {
        return;
    }
    let mut moved = Vec::new();
    let mut keep = Vec::with_capacity(candidates.len());
    for (i, c) in candidates.drain(..).enumerate() {
        if i < last && yields(&c) {
            moved.push(c);
        } else {
            keep.push(c);
        }
    }
    let at = keep
        .iter()
        .rposition(exact_word)
        .map_or(keep.len(), |i| i + 1);
    keep.splice(at..at, moved);
    make_scores_monotonic(&mut keep);
    *candidates = keep;
}

/// 两个候选的音节一样（整词和拼凑出来的整句：启动器 / 启动 | 其）。
pub(crate) fn same_syllables(a: &Candidate, b: &Candidate) -> bool {
    a.pinyin.split_whitespace().eq(b.pinyin.split_whitespace())
}

/// 不靠纠错、补全、整个输入是一个够常用（[`EXACT_PROTECT_MIN_FREQ`]）的词库词的候选里，最少几个音节。
pub(crate) fn fewest_exact_syllables(candidates: &[Candidate]) -> Option<usize> {
    candidates
        .iter()
        .filter(|c| {
            c.words.len() == 1
                && c.words[0].known
                && c.words[0].freq >= EXACT_PROTECT_MIN_FREQ
                && c.features.typo_count == 0.0
                && c.features.prefix_completion == 0.0
                && c.features.coverage_score >= 1.0
        })
        .map(|c| c.pinyin.split_whitespace().count())
        .min()
}

/// 只打一个声母（l、b、zh）时固定排第一的字：词库按读音分的词频把它们排得太后
/// （了 le 348 万，来 1484 万；不 299 万，把 380 万），而且第一个候选不该随上文、选过的字变来变去
/// （#156）。没列的声母照词频排。
const SINGLE_INITIAL_TOP: &[(&str, &str)] = &[
    ("b", "不"),
    ("d", "的"),
    ("g", "个"),
    ("h", "和"),
    ("j", "就"),
    ("l", "了"),
    ("n", "你"),
    ("q", "去"),
    ("r", "人"),
    ("s", "是"),
    ("sh", "是"),
    ("t", "他"),
    ("w", "我"),
    ("y", "一"),
    ("z", "在"),
    ("zh", "这"),
];

/// 输入只是一个声母时，把 [`SINGLE_INITIAL_TOP`] 里的字挪到第一个（在选过最多的字前面；
/// 自定义词、自定义编码之后还会排到它前面）。候选里没有这个字就不动。
///
/// 第一个已经是常选的字（[`promote_picked`] 按 [`strongly_picked`] 提上来的，比如这几天一直在打「赖」）
/// 时不动：首选跟着用户最近的习惯变，不再死钉着「了」（#156）。
fn pin_single_initial(candidates: &mut Vec<Candidate>, input: &str) {
    let Some(&(_, top)) = SINGLE_INITIAL_TOP.iter().find(|(key, _)| *key == input) else {
        return;
    };
    if let Some(i) = candidates
        .iter()
        .position(|c| c.text == top && c.words.len() == 1)
    {
        // 只打一个声母时候选都是简拼出来的（is_picked 不认），这里单独看：最近常选的单字排第一
        let strong = candidates
            .iter()
            .enumerate()
            .filter(|(j, c)| {
                *j != i
                    && c.text.chars().count() == 1
                    && c.features.typo_count == 0.0
                    && c.features.history_use_count >= STRONG_PICK_HEAT
                    && c.features.history_use_count
                        >= 2.0 * candidates[i].features.history_use_count.max(0.5)
            })
            .max_by(|a, b| {
                a.1.features
                    .history_use_count
                    .total_cmp(&b.1.features.history_use_count)
            })
            .map(|(j, _)| j);
        if let Some(j) = strong {
            let c = candidates.remove(j);
            candidates.insert(0, c);
            return;
        }
        let pinned = candidates.remove(i);
        candidates.insert(0, pinned);
    }
}

/// 用户上次给这串输入选过的候选往前放（微信输入法的 FirstUserCandidateRanker /
/// SingleUserWordCanbeSecond）。
///
/// - 词和整句：选得最多的那个放第一（第一个候选被选得更多时不动）。
/// - 单字：第一个候选一般不动——偶尔选一次生僻字，不该让它以后一直顶掉这个音节最常用的字；
///   选过的字按热度从第二位起排（#161：找过一次的字下次就在第一页，不只提选得最多的那一个），
///   最多 [`PICKED_CHARS_MAX`] 个，免得几次手滑把常用字挤到第二页。
///   但最近常选的字（[`strongly_picked`]：选了好几次、比第一个选得多得多）排第一，首选跟着习惯变（#156）；
///   热度按 30 天半衰，不再选了就慢慢退回去，又选回原来的字时更快。
///   两样都有时（xian 选过「西安」也选过「先」）词照样放第一，字跟在后面。
///
/// 只看「这个候选本身被选过」（不算末词命中），而且要精确匹配（不是纠错、补全出来的）。
/// 热度（`history_use_count`，衰减过的）不到 [`learn::MIN_PICK_HEAT`] 的不算：
/// 误选一次的词一个月以后就不再一直顶在前面。
fn promote_picked(candidates: &mut Vec<Candidate>, _input: &str) {
    let single = |c: &Candidate| c.text.chars().count() <= 1;
    let heat = |c: &Candidate| c.features.history_use_count;
    let mut changed = false;
    // 词和整句：选得最多的放第一（一样多时取靠前的）
    let best_word = candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| !single(c) && is_picked(c))
        .max_by(|a, b| heat(a.1).total_cmp(&heat(b.1)).then_with(|| b.0.cmp(&a.0)))
        .map(|(i, _)| i);
    if let Some(best) = best_word.filter(|&i| i > 0) {
        // 第一个候选被选得更多（比如常选的单字），就不动它
        if heat(&candidates[0]) < heat(&candidates[best]) {
            let c = candidates.remove(best);
            candidates.insert(0, c);
            changed = true;
        }
    }
    // 单字：第一个候选之后，按热度排选过的字，最多 PICKED_CHARS_MAX 个
    let mut picked: Vec<usize> = (1..candidates.len())
        .filter(|&i| single(&candidates[i]) && is_picked(&candidates[i]))
        .collect();
    // 稳定排序：热度一样时保持原来（按词频）的先后
    picked.sort_by(|&a, &b| heat(&candidates[b]).total_cmp(&heat(&candidates[a])));
    picked.truncate(PICKED_CHARS_MAX);
    if !picked.is_empty() {
        let moved: Vec<Candidate> = picked.iter().map(|&i| candidates[i].clone()).collect();
        let mut rest = Vec::with_capacity(candidates.len());
        for (i, c) in std::mem::take(candidates).into_iter().enumerate() {
            if !picked.contains(&i) {
                rest.push(c);
            }
        }
        let first_strong = rest
            .first()
            .is_some_and(|first| strongly_picked(&moved[0], first));
        let at = if first_strong { 0 } else { rest.len().min(1) };
        rest.splice(at..at, moved);
        *candidates = rest;
        changed = true;
    }
    if changed {
        make_scores_monotonic(candidates);
    }
}

/// [`promote_picked`]：选过的单字最多提前几个。
const PICKED_CHARS_MAX: usize = 3;

/// 一个字选了这么多次（衰减过的热度）以后，才可能顶掉第一个候选（[`strongly_picked`]）。
/// 选个三五次只排第二（#161 找过的字在第一页就行）；连着打一阵子（礼单上全是「赖」）才换首选（#156）。
const STRONG_PICK_HEAT: f32 = 6.0;

/// `picked` 是最近常选的字，该排到 `first` 前面：热度到 [`STRONG_PICK_HEAT`]，而且是 `first` 的两倍以上
/// （`first` 自己也常选时就不换，免得两个字来回跳）。误选一两次不算。
fn strongly_picked(picked: &Candidate, first: &Candidate) -> bool {
    let heat = picked.features.history_use_count;
    is_picked(picked)
        && heat >= STRONG_PICK_HEAT
        && heat >= 2.0 * first.features.history_use_count.max(0.5)
}

/// 这个候选本身被精确选过（不是靠末词命中、纠错或补全出来的），而且热度够
/// [`learn::MIN_PICK_HEAT`]（误选一次、过了一个月就不算）。
fn is_picked(c: &Candidate) -> bool {
    f64::from(c.features.history_use_count) >= learn::MIN_PICK_HEAT
        && c.features.typo_count == 0.0
        && c.features.prefix_completion == 0.0
        && c.features.full_jianpin == 0.0
}

/// 分数改成单调递减，后面再按分数重排（繁体转换）顺序也不变。
fn make_scores_monotonic(candidates: &mut [Candidate]) {
    for i in 1..candidates.len() {
        if candidates[i].score >= candidates[i - 1].score {
            candidates[i].score = candidates[i - 1].score - 1e-3;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rank::Feature;

    fn word(text: &str, pinyin: &str, freq: f64) -> CandidateWord {
        CandidateWord {
            text: text.to_string(),
            pinyin: pinyin.to_string(),
            freq,
            known: true,
        }
    }

    fn cand(text: &str, pinyin: &str, score: f64, freq: f64) -> Candidate {
        Candidate {
            pinyin: pinyin.to_string(),
            score,
            features: Feature::default(),
            words: vec![word(text, pinyin, freq)],
            consumed: pinyin.len(),
            text: text.to_string(),
        }
    }

    /// 打全的整词（可以当锚点）。
    fn exact(text: &str, pinyin: &str, score: f64) -> Candidate {
        let mut c = cand(text, pinyin, score, 9000.0);
        c.features.coverage_score = 1.0;
        c
    }

    /// 补全出来的候选。
    fn completion(text: &str, pinyin: &str, score: f64) -> Candidate {
        let mut c = cand(text, pinyin, score, 8000.0);
        c.features.prefix_completion = 5.0;
        c
    }

    fn texts(list: &[Candidate]) -> Vec<&str> {
        list.iter().map(|c| c.text.as_str()).collect()
    }

    #[test]
    fn steps_keep_candidate_count() {
        let mut list = vec![
            completion("想要", "xiang yao", 100.0),
            exact("中国", "zhong guo", 50.0),
            cand("重点", "zhong dian", 40.0, 7000.0),
        ];
        list[2].features.typo_count = 1.0;
        list[1].features.fuzzy = true;
        for (name, step) in STEPS {
            let mut copy = list.clone();
            step(&mut copy, "zhongguo");
            assert_eq!(copy.len(), list.len(), "步骤「{name}」改变了候选数量");
        }
        let mut copy = list.clone();
        stabilize_input(&mut copy, "zhongguo");
        assert_eq!(copy.len(), list.len());
    }

    #[test]
    fn full_word_goes_before_completion() {
        let mut list = vec![
            completion("想要", "xiang yao", 100.0),
            exact("中国", "zhong guo", 50.0),
        ];
        protect_exact_input(&mut list, "zhongguo");
        assert_eq!(texts(&list), ["中国", "想要"]);
    }

    #[test]
    fn typo_yields_to_typed_reading() {
        let mut typo = cand("重点", "zhong dian", 100.0, 9000.0);
        typo.features.typo_count = 1.0;
        let mut list = vec![typo, exact("种地", "zhong di", 50.0)];
        typos_after_typed(&mut list, "zhongdi");
        assert_eq!(texts(&list), ["种地", "重点"]);
    }

    #[test]
    fn completion_goes_after_last_exact_word() {
        let mut list = vec![
            completion("想要", "xiang yao", 100.0),
            exact("想法", "xiang fa", 60.0),
            exact("香油", "xiang you", 30.0),
        ];
        completions_after_exact_words(&mut list, "xiang?");
        assert_eq!(texts(&list), ["想法", "香油", "想要"]);
    }

    #[test]
    fn fuzzy_goes_after_exact_word() {
        let mut fuzzy = cand("朋友", "peng you", 100.0, 9000.0);
        fuzzy.features.fuzzy = true;
        let mut list = vec![fuzzy, exact("喷油", "pen you", 50.0)];
        fuzzy_after_exact_word(&mut list, "penyou");
        assert_eq!(texts(&list), ["喷油", "朋友"]);
    }

    #[test]
    fn single_initial_pins_top_char() {
        let mut list = vec![
            cand("在", "z", 100.0, 9000.0),
            cand("了", "l", 50.0, 8000.0),
        ];
        pin_single_initial(&mut list, "l");
        assert_eq!(texts(&list), ["了", "在"]);
    }

    #[test]
    fn picked_char_moves_up_but_keeps_first() {
        let mut picked = cand("国", "guo", 50.0, 5000.0);
        picked.features.history_use_count = 4.0;
        let mut list = vec![cand("中", "zhong", 100.0, 9000.0), picked];
        promote_picked(&mut list, "guo");
        assert_eq!(texts(&list), ["中", "国"], "热度不到两倍时首选不换");
    }

    #[test]
    fn strongly_picked_char_takes_first() {
        let mut picked = cand("国", "guo", 50.0, 5000.0);
        picked.features.history_use_count = 8.0;
        let mut list = vec![cand("中", "zhong", 100.0, 9000.0), picked];
        promote_picked(&mut list, "guo");
        assert_eq!(texts(&list), ["国", "中"]);
    }

    #[test]
    fn stabilize_is_deterministic() {
        let build = || {
            let mut list = vec![
                completion("想要", "xiang yao", 100.0),
                exact("想法", "xiang fa", 60.0),
                exact("香油", "xiang you", 30.0),
            ];
            list[1].features.fuzzy = true;
            list
        };
        let (mut a, mut b) = (build(), build());
        stabilize_input(&mut a, "xiang?");
        stabilize_input(&mut b, "xiang?");
        assert_eq!(texts(&a), texts(&b));
        assert_eq!(a.len(), b.len());
    }
}
