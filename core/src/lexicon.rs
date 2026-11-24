//! 词典抽象与内存实现。
//!
//! 真实 WeType 词库是 LOUDS trie + 音节 ID 编码，后续解析器只需实现
//! Lexicon trait 即可接入；这里的 MemoryLexicon 同时提供 TSV 加载，便于
//! 先用导出的词表跑通整条链路。
//!
//! TSV 格式：拼音<TAB>词<TAB>频次
//! - 多音节拼音用空格或隔音符分隔，例如：xi an / xi'an
//! - 儿化用独立的 r 标记，例如：hua r
//! - 以 # 开头的行和空行会被忽略。

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};

use anyhow::{anyhow, bail, Context, Result};

use crate::lexstore::{rare_last, Arr, Blob, FpSet, IdTable, Postings, MAX_IDS};
use crate::syllable::{Syllable, SyllableId, SyllableQuery};
use crate::sync::write_lock;

/// 词库快照的魔数与格式版本。格式一变就改版本号，旧快照自动失效。
///
/// 3：各列数组原样写下来、载入时映射进来直接用（#233）；2 以前是一条一条的词条。
const CACHE_MAGIC: &[u8; 4] = b"WLC1";
const CACHE_VERSION: u32 = 3;
/// 快照头的定长部分（后面接 [`CACHE_SECTIONS`] 个「偏移、字节数」）。
const CACHE_HEADER: usize = 48;
/// 快照里有几节数组，见 `MemoryLexicon::sections`。
const CACHE_SECTIONS: usize = 20;

/// 快照里存的是音节 ID（音节表的下标），音节表一改（比如补了 biang）旧快照就对不上了。
/// 所以写进快照的版本号混进音节表的指纹：表一变，旧快照自动作废重建。
fn cache_version() -> u32 {
    // FNV-1a
    let mut hash: u32 = 0x811c_9dc5;
    for text in crate::syllable::SYLLABLES {
        for &byte in text.as_bytes().iter().chain(std::iter::once(&b',')) {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
    }
    CACHE_VERSION ^ (hash & 0xffff_ff00)
}

/// 占位频次的词估频时给的成词加成（自然对数，约 ×20）。见 `estimate_placeholder_freqs`。
const PLACEHOLDER_BONUS: f64 = 3.0;
/// 同一个频次至少有这么多多音节词才当占位值（小词库、用户词库不处理）。
const PLACEHOLDER_MIN_WORDS: usize = 1000;
/// 切分时每一部分最多几个音节。
const PLACEHOLDER_MAX_PART: usize = 6;

fn cache_path(path: &Path) -> std::path::PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(".wcache");
    std::path::PathBuf::from(os)
}

/// 快照可能落的位置：先放词库旁边，写不进去（比如 /usr/share 只读）
/// 再放 XDG 缓存目录。
fn cache_paths(path: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![cache_path(path)];
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".cache"))
        });
    if let Some(base) = base {
        // 用 FNV-1a 给路径算个短名字，避免不同词库撞在一起
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in path.to_string_lossy().as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        out.push(base.join("archtype").join(format!("{hash:016x}.wcache")));
    }
    out
}

/// 源文件的 (大小, 修改时间秒)。拿不到就不用缓存。
fn source_stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some((meta.len(), mtime))
}

const KEY_HASH_SEED: u64 = 0xcbf2_9ce4_8422_2325;

/// 音节键序列的滚动哈希（FNV-1a）。
fn key_hash_step(hash: u64, key: SyllableKey) -> u64 {
    let raw = u64::from(key.id) | (u64::from(key.erhua) << 16);
    let mut h = hash;
    for byte in raw.to_le_bytes().iter().take(3) {
        h ^= u64::from(*byte);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// 扩展 B 区以后、常见中文字体（Noto CJK、思源）带的字：通用规范汉字表里的 196 个加雾凇
/// 另收的几个（𰻝）。按码位排好序。其余的见 [`LexEntry::is_ext_char`]。
const COMMON_EXT_CHARS: &str = concat!(
    "𠅤𠙶𠳐𡎚𡐓𣗋𣲗𣲘𣸣𤆵𤧛𤩽𤫉𥔲𥕢𥖨𥻗𦈡𦒍𦙶𦝼𦭜𦰡𧿹𨐈𨙸𨚕𨟠𨭉𨱇𨱏𨱑𨱔𨺙𩽾𩾃𩾌𪟝𪣻𪤗",
    "𪨰𪨶𪩘𪾢𫄧𫄨𫄷𫄸𫇭𫌀𫍣𫍯𫍲𫍽𫐄𫐐𫐓𫑡𫓧𫓯𫓶𫓹𫔍𫔎𫔶𫖮𫖯𫖳𫗧𫗴𫘜𫘝𫘦𫘧𫘨𫘪𫘬𫚕𫚖𫚭",
    "𫛭𫞩𫟅𫟦𫟷𫟹𫟼𫠆𫠊𫠜𫢸𫫇𫭟𫭢𫭼𫮃𫰛𫵷𫶇𫷷𫸩𬀩𬀪𬂩𬃊𬇕𬇙𬇹𬉼𬊈𬊤𬌗𬍛𬍡𬍤𬒈𬒔𬒗𬕂𬘓",
    "𬘘𬘡𬘩𬘫𬘬𬘭𬘯𬙂𬙊𬙋𬜬𬜯𬞟𬟁𬟽𬣙𬣞𬣡𬣳𬤇𬤊𬤝𬨂𬨎𬩽𬪩𬬩𬬭𬬮𬬱𬬸𬬹𬬻𬬿𬭁𬭊𬭎𬭚𬭛𬭤",
    "𬭩𬭬𬭯𬭳𬭶𬭸𬭼𬮱𬮿𬯀𬯎𬱖𬱟𬳵𬳶𬳽𬳿𬴂𬴃𬴊𬶋𬶍𬶏𬶐𬶟𬶠𬶨𬶭𬶮𬷕𬸘𬸚𬸣𬸦𬸪𬹼𬺈𬺓𰻝",
);

/// 词典条目。
#[derive(Clone, Debug, PartialEq)]
pub struct LexEntry {
    pub word: String,
    pub freq: f64,
    /// 规范音节键（全拼，含儿化标记）。
    pub keys: Vec<SyllableKey>,
    /// 每个音节的首字母，用于简拼匹配。
    pub initials: Vec<u8>,
}

/// [`LexEntry::is_rare`] 的判定，给手里只有词和频次的地方用。
pub fn is_rare_char(word: &str, freq: f64) -> bool {
    freq < 1.0 && word.chars().count() == 1
}

impl LexEntry {
    /// 生僻字：频次不到 1 的单字（大字表补进来的读音，见 tools/rime2tsv.py --extra-chars）。
    ///
    /// 它们不参与组句、不占前几页，只在翻页的长尾里列出来，保证一个音节的字都找得到。
    /// 一个音节动辄几百个这样的字（yi 有六百多），让束搜索每一步都去排它们会慢很多。
    pub fn is_rare(&self) -> bool {
        is_rare_char(&self.word, self.freq)
    }

    /// CJK 扩展 B 区及以后（U+20000 起）的单字，通用规范汉字表里的除外（[`COMMON_EXT_CHARS`]）。
    ///
    /// 常见中文字体都不带这些字，列出来是方块，默认不出，见
    /// [`crate::BeamConfig::rare_ext_chars`]。不看频次：万象词库把它们都记成普通单字。
    /// 含这些字的词（𩽾𩾌）不算，那是词库特意收的。
    pub fn is_ext_char(&self) -> bool {
        is_ext_char(&self.word)
    }
}

/// [`LexEntry::is_ext_char`]，给手里只有词的地方用（[`Lexicon::visit`]）。
pub fn is_ext_char(word: &str) -> bool {
    // 四字节 UTF-8 且只有一个字
    if word.len() != 4 || word.as_bytes()[0] < 0xF0 {
        return false;
    }
    let bytes = word.as_bytes();
    // U+20000 起的 UTF-8 是 F0 A0 80 80 起；四字节 UTF-8 按字节比和按码位比先后一样
    if bytes < &[0xF0, 0xA0, 0x80, 0x80][..] {
        return false;
    }
    // COMMON_EXT_CHARS 全是四字节的字、按码位排好序：二分，不逐个比（万象词库一个声母的单字里
    // 这样的字有几百个，束搜索每键要判断上万次，#227）
    let table = COMMON_EXT_CHARS.as_bytes();
    let (mut lo, mut hi) = (0, table.len() / 4);
    while lo < hi {
        let mid = (lo + hi) / 2;
        match table[mid * 4..mid * 4 + 4].cmp(bytes) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => return false,
        }
    }
    true
}

/// 全拼音节键。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyllableKey {
    pub id: SyllableId,
    pub erhua: bool,
}

impl SyllableKey {
    pub fn new(id: SyllableId, erhua: bool) -> Self {
        Self { id, erhua }
    }
}

/// 词典接口。
///
/// query 是一段连续音节；返回所有“音节序列恰好等于 query”的词。
/// 简拼查询用 SyllableQuery::Jianpin，匹配每个音节的首字母。
pub trait Lexicon: Send + Sync {
    /// 返回所有权值而非引用：这样「基础词库 + 用户词库」可以合并返回，
    /// 也能在运行时安全地增删用户词。命中数很小，克隆成本可接受。
    fn lookup(&self, query: &[SyllableQuery]) -> Vec<LexEntry>;

    /// 同 [`Lexicon::lookup`]，但不要生僻字（[`LexEntry::is_rare`]）。组句用这个。
    fn lookup_common(&self, query: &[SyllableQuery]) -> Vec<LexEntry> {
        let mut found = self.lookup(query);
        found.retain(|e| !e.is_rare());
        found
    }

    /// 模糊音查询（z/zh、an/ang 等）。groups 是打开的分组（`syllable::fuzzy::*` 位掩码）。
    /// 默认不支持。
    fn lookup_fuzzy(&self, _query: &[SyllableQuery], _groups: u32) -> Vec<LexEntry> {
        Vec::new()
    }

    /// 把模糊音查询要用的索引先建好（载入词库的后台线程里调，免得第一次按键时现建）。默认什么都不做。
    fn prepare_fuzzy(&self) {}

    /// 是否存在比 query 更长、且以 query 开头的词。
    ///
    /// 束搜索靠它剪掉不可能成词的音节串；不确定时返回 true（只影响速度，不影响结果）。
    fn has_longer(&self, _query: &[SyllableQuery]) -> bool {
        true
    }

    fn total_freq(&self) -> f64;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 词的一元代价（越大越不可能）。缺省用 -ln(freq/total)，见 [`freq_cost`]。
    fn cost(&self, entry: &LexEntry) -> f64 {
        freq_cost(entry.freq, self.total_freq())
    }

    /// 和 [`Lexicon::lookup`]（`common` 时 [`Lexicon::lookup_common`]）查到的词一样、先后一样，
    /// 只是不复制词条，一条一条把（词, 频次）交给 `f`。
    ///
    /// 束搜索每键要查几百次词典（#227）：简拼 `s`、`sd` 在万象词库里一次就是几千个词，
    /// 每个词条复制一遍（字、音节、声母三份内存分配）比查表本身慢得多，而大部分马上就被截掉了。
    /// 缺省实现就是查完再逐条给，内存词典、用户词库自己实现成不复制的。
    fn visit(&self, query: &[SyllableQuery], common: bool, f: &mut dyn FnMut(&str, f64)) {
        let found = if common {
            self.lookup_common(query)
        } else {
            self.lookup(query)
        };
        found.iter().for_each(|e| f(&e.word, e.freq));
    }
}

/// [`Lexicon::cost`] 的缺省算法：-ln(freq / total)，频次至少按 0.5、总频次至少按 1 算。
/// 束搜索一次查几千个词时先取好 total（用户词库取一次要拿两把读锁），再逐个用它算，结果一位不差。
pub fn freq_cost(freq: f64, total_freq: f64) -> f64 {
    let total = total_freq.max(1.0);
    -((freq.max(0.5)) / total).ln()
}

/// 音节键在索引、快照里的写法：低 15 位是音节 ID，最高位是儿化。
#[inline]
fn raw_key(key: SyllableKey) -> u16 {
    (key.id & 0x7fff) | (u16::from(key.erhua) << 15)
}

#[inline]
fn key_of_raw(raw: u16) -> SyllableKey {
    SyllableKey::new(raw & 0x7fff, raw & 0x8000 != 0)
}

/// 音节的首字母（简拼那一维），和 `Syllable::initial_letter` 一样。
#[inline]
fn initial_of(raw: u16) -> u8 {
    crate::syllable::SYLLABLES
        .get(usize::from(raw & 0x7fff))
        .map_or(0, |s| s.as_bytes()[0])
}

/// 一串音节键的哈希（has_longer 的前缀哈希也是这样一个一个滚出来的）。
#[inline]
fn raw_hash(keys: &[u16]) -> u64 {
    keys.iter()
        .fold(KEY_HASH_SEED, |h, &raw| key_hash_step(h, key_of_raw(raw)))
}

#[inline]
fn initials_hash(letters: &[u8]) -> u64 {
    letters
        .iter()
        .fold(KEY_HASH_SEED, |h, &letter| initial_hash_step(h, letter))
}

#[inline]
fn fuzzy_hash(keys: impl Iterator<Item = u32>) -> u64 {
    keys.fold(KEY_HASH_SEED, |h, k| {
        (h ^ u64::from(k)).wrapping_mul(0x1000_0000_01b3)
    })
}

/// 同一种读音（全拼音节串，或者简拼的声母串）的词条一组。组 g 的键是 `keys[key_end[g-1]..key_end[g]]`，
/// 组里的词条下标在 `post`（加词的先后，生僻字排在最后），按键找组用 `table`。
#[derive(Default)]
struct Groups<K: Copy> {
    key_end: Arr<u32>,
    keys: Arr<K>,
    post: Postings,
    table: IdTable,
}

impl<K: Copy + PartialEq> Groups<K> {
    fn len(&self) -> usize {
        self.key_end.len()
    }

    #[inline]
    fn key(&self, g: u32) -> &[K] {
        let g = g as usize;
        let start = if g == 0 {
            0
        } else {
            self.key_end[g - 1] as usize
        };
        &self.keys[start..self.key_end[g] as usize]
    }

    /// 键正好是 `key` 的组。
    #[inline]
    fn find(&self, hash: u64, key: &[K]) -> Option<u32> {
        self.table.find(hash, |g| self.key(g) == key)
    }

    /// 接在最后加一组（调用方保证还没有）。
    fn add(&mut self, key: &[K], hash_of: impl Fn(&[K]) -> u64) -> u32 {
        let g = self.key_end.len() as u32;
        self.keys.to_mut().extend_from_slice(key);
        self.key_end.push(self.keys.len() as u32);
        let Groups {
            key_end,
            keys,
            table,
            ..
        } = self;
        let key_at = |g: u32| {
            let g = g as usize;
            let start = if g == 0 { 0 } else { key_end[g - 1] as usize };
            &keys[start..key_end[g] as usize]
        };
        table.insert(hash_of(key), g, |old| hash_of(key_at(old)));
        g
    }

    /// 键的偏移单调、最后一个是键的总数，每组至少一个键。
    fn check_keys(&self) -> bool {
        crate::lexstore::strictly_increasing(&self.key_end, self.keys.len())
    }

    fn heap_bytes(&self) -> usize {
        self.key_end.heap_bytes()
            + self.keys.heap_bytes()
            + self.post.heap_bytes()
            + self.table.heap_bytes()
    }
}

/// 模糊音索引：键是各音节在全部分组都打开时的归并键（`syllable::fuzzy_key`）。
/// 一个归并键一组，组里是词条下标（按加词的先后，不把生僻字挪到后面）；组的键不另存，
/// 取组里第一个全拼读音组（`rep`）的音节现算。
#[derive(Default)]
struct FuzzyIndex {
    rep: Arr<u32>,
    post: Postings,
    table: IdTable,
}

/// TSV 解析出来、还没建索引的词条。
#[derive(Default)]
struct Parsed {
    words: Vec<u8>,
    word_end: Vec<u32>,
    freqs: Vec<f64>,
    key_end: Vec<u32>,
    keys: Vec<u16>,
    total_freq: f64,
    skipped: usize,
}

/// 内存词典。
///
/// 存法见 [`crate::lexstore`]：一列一列的数组，不是一个词条一个结构。词的字连成一整块，
/// 全拼读音一样的词条共用一组音节（存一次），声母由音节现算。从快照载入时这些数组直接指着
/// 映射进来的文件。
#[derive(Default)]
pub struct MemoryLexicon {
    /// 所有词的字连在一起：第 i 个词是 `words[word_end[i-1]..word_end[i]]`。
    words: Arr<u8>,
    word_end: Arr<u32>,
    freqs: Arr<f64>,
    /// 词条 -> 全拼读音组（`full` 的组号）。
    group: Arr<u32>,
    full: Groups<u16>,
    jianpin: Groups<u8>,
    /// 模糊音索引，见 [`FuzzyIndex`]。
    ///
    /// 默认不构建——模糊音默认关闭；第一次模糊查询或 `prepare_fuzzy` 时再建。快照里带着一份。
    fuzzy_index: OnceLock<FuzzyIndex>,
    /// 全拼读音组所有「真前缀」的哈希，供 has_longer 剪枝（哈希碰撞只会少剪，不会错剪）。
    full_prefixes: FpSet,
    /// 同上，按声母：简拼和「全拼混简拼」（kyi、yongzwen）的查询用它剪枝。
    initial_prefixes: FpSet,
    total_freq: f64,
    /// 加载时被跳过的行数（未知音节等）。
    skipped: usize,
    /// 快照映射进来的那块内存：上面 `Arr::Mapped` 的数组都指着它，所以放在最后（最后释放）。
    blob: Option<Arc<Blob>>,
}

/// 词条能不能收：词不空、频次是非负有限数、拼音解析得出来。返回音节键。
fn check_entry(pinyin: &str, word: &str, freq: f64) -> Result<Vec<SyllableKey>> {
    if word.is_empty() {
        bail!("词不能为空");
    }
    if !freq.is_finite() || freq < 0.0 {
        bail!("频次必须是非负有限数，收到 {freq}");
    }
    let keys = parse_pinyin_keys(pinyin)?;
    for k in &keys {
        if Syllable::from_id(k.id).is_none() {
            bail!("未知音节 ID: {}", k.id);
        }
    }
    Ok(keys)
}

impl MemoryLexicon {
    pub fn new() -> Self {
        Self::default()
    }

    /// 第 i 个词条的字。
    #[inline]
    fn word(&self, i: u32) -> &str {
        let i = i as usize;
        let start = if i == 0 {
            0
        } else {
            self.word_end[i - 1] as usize
        };
        let bytes = &self.words[start..self.word_end[i] as usize];
        // 安全：建的时候每个词都是 &str 拷进来的；快照载入时逐个检查过是合法 UTF-8（read_cache）
        unsafe { std::str::from_utf8_unchecked(bytes) }
    }

    #[inline]
    fn freq(&self, i: u32) -> f64 {
        self.freqs[i as usize]
    }

    /// [`LexEntry::is_rare`]。
    #[inline]
    fn is_rare_at(&self, i: u32) -> bool {
        self.freq(i) < 1.0 && is_single_char(self.word(i))
    }

    /// 第 i 个词条的音节键（索引里的写法）。
    #[inline]
    fn keys_at(&self, i: u32) -> &[u16] {
        self.full.key(self.group[i as usize])
    }

    /// 第 i 个词条，复制成 [`LexEntry`]。
    fn entry(&self, i: u32) -> LexEntry {
        let keys = self.keys_at(i);
        LexEntry {
            word: self.word(i).to_string(),
            freq: self.freq(i),
            keys: keys.iter().map(|&raw| key_of_raw(raw)).collect(),
            initials: keys.iter().map(|&raw| initial_of(raw)).collect(),
        }
    }

    /// 插入一个词。pinyin 使用空格/隔音符分隔音节。
    pub fn insert(&mut self, pinyin: &str, word: &str, freq: f64) -> Result<()> {
        let keys = check_entry(pinyin, word, freq)?;
        let raw: Vec<u16> = keys.iter().map(|&k| raw_key(k)).collect();
        self.add_entry(&raw, word, freq);
        Ok(())
    }

    /// 建好以后再加一个词条：组不存在就接在最后新开一组，下标记进 `extra`。
    fn add_entry(&mut self, raw: &[u16], word: &str, freq: f64) {
        let idx = self.freqs.len() as u32;
        self.words.to_mut().extend_from_slice(word.as_bytes());
        self.word_end.push(self.words.len() as u32);
        self.freqs.push(freq);

        let g = match self.full.find(raw_hash(raw), raw) {
            Some(g) => g,
            None => {
                let mut hash = KEY_HASH_SEED;
                for &key in &raw[..raw.len() - 1] {
                    hash = key_hash_step(hash, key_of_raw(key));
                    self.full_prefixes.insert(hash);
                }
                self.full.add(raw, raw_hash)
            }
        };
        self.group.push(g);
        let initials: Vec<u8> = raw.iter().map(|&k| initial_of(k)).collect();
        let jp = match self.jianpin.find(initials_hash(&initials), &initials) {
            Some(jp) => jp,
            None => {
                let mut hash = KEY_HASH_SEED;
                for &letter in &initials[..initials.len() - 1] {
                    hash = initial_hash_step(hash, letter);
                    self.initial_prefixes.insert(hash);
                }
                self.jianpin.add(&initials, initials_hash)
            }
        };
        // 每组里生僻字（LexEntry::is_rare）排在最后，lookup_common 读到它们就停
        let rare = self.is_rare_at(idx);
        let mut lists = [
            self.full.post.extra.remove(&g).unwrap_or_default(),
            self.jianpin.post.extra.remove(&jp).unwrap_or_default(),
        ];
        for list in &mut lists {
            list.push(idx);
            if !rare {
                let mut at = list.len() - 1;
                while at > 0 && self.is_rare_at(list[at - 1]) {
                    list.swap(at - 1, at);
                    at -= 1;
                }
            }
        }
        let [full, jianpin] = lists;
        self.full.post.extra.insert(g, full);
        self.jianpin.post.extra.insert(jp, jianpin);
        self.total_freq += freq;
        // 模糊音索引是按当时的词条建的，多了一个词就不全了：丢掉，下次模糊查询再建
        self.fuzzy_index.take();
    }

    /// 解析好的词条一次建好全部索引（TSV 载入走这里）。
    fn build(p: Parsed) -> Result<Self> {
        let n = p.freqs.len();
        if n >= u32::MAX as usize || p.words.len() >= u32::MAX as usize {
            bail!("词库太大");
        }
        let ekey = |e: u32| {
            let e = e as usize;
            let start = if e == 0 { 0 } else { p.key_end[e - 1] as usize };
            &p.keys[start..p.key_end[e] as usize]
        };
        let is_rare = |e: u32| {
            let e = e as usize;
            let start = if e == 0 {
                0
            } else {
                p.word_end[e - 1] as usize
            };
            p.freqs[e] < 1.0
                && std::str::from_utf8(&p.words[start..p.word_end[e] as usize])
                    .is_ok_and(is_single_char)
        };
        let mut scratch = Vec::new();

        // 全拼：按音节串排（稳定排序，同组里还是加词的先后），一段一组
        let mut order: Vec<u32> = (0..n as u32).collect();
        order.sort_by(|&a, &b| ekey(a).cmp(ekey(b)));
        let mut full = Groups::<u16>::default();
        let (mut key_end, mut keys, mut post_end) = (Vec::new(), Vec::new(), Vec::new());
        let mut group = vec![0u32; n];
        for (pos, &e) in order.iter().enumerate() {
            if pos == 0 || ekey(order[pos - 1]) != ekey(e) {
                if pos > 0 {
                    post_end.push(pos as u32);
                }
                keys.extend_from_slice(ekey(e));
                key_end.push(keys.len() as u32);
            }
            group[e as usize] = key_end.len() as u32 - 1;
        }
        if n > 0 {
            post_end.push(n as u32);
        }
        let groups = key_end.len();
        if groups >= MAX_IDS {
            bail!("词库太大");
        }
        let mut start = 0usize;
        for &end in &post_end {
            rare_last(&mut order[start..end as usize], is_rare, &mut scratch);
            start = end as usize;
        }
        full.key_end = key_end.into();
        full.keys = keys.into();
        full.post.end = post_end.into();
        full.post.ids = order.into();
        full.table = IdTable::with_capacity(groups);
        let mut full_prefixes = FpSet::default();
        for g in 0..groups as u32 {
            let key = full.key(g);
            let mut hash = KEY_HASH_SEED;
            for &raw in &key[..key.len() - 1] {
                hash = key_hash_step(hash, key_of_raw(raw));
                full_prefixes.insert(hash);
            }
            let hash = raw_hash(key);
            full.table.insert(hash, g, |_| unreachable!("容量够"));
        }
        full_prefixes.compact();

        // 简拼：每个全拼组的声母串，按声母串排，一段一组；组里的词条按下标（加词的先后）
        let mut gi_end = Vec::with_capacity(groups);
        let mut gi = Vec::with_capacity(full.keys.len());
        for g in 0..groups as u32 {
            gi.extend(full.key(g).iter().map(|&raw| initial_of(raw)));
            gi_end.push(gi.len() as u32);
        }
        let ginit = |g: u32| {
            let g = g as usize;
            let start = if g == 0 { 0 } else { gi_end[g - 1] as usize };
            &gi[start..gi_end[g] as usize]
        };
        let mut gorder: Vec<u32> = (0..groups as u32).collect();
        gorder.sort_unstable_by(|&a, &b| ginit(a).cmp(ginit(b)).then(a.cmp(&b)));
        let mut jianpin = Groups::<u8>::default();
        let (mut key_end, mut keys) = (Vec::new(), Vec::new());
        let mut jp_of_group = vec![0u32; groups];
        for (pos, &g) in gorder.iter().enumerate() {
            if pos == 0 || ginit(gorder[pos - 1]) != ginit(g) {
                keys.extend_from_slice(ginit(g));
                key_end.push(keys.len() as u32);
            }
            jp_of_group[g as usize] = key_end.len() as u32 - 1;
        }
        drop(gorder);
        let jp_groups = key_end.len();
        jianpin.key_end = key_end.into();
        jianpin.keys = keys.into();
        let (post_end, mut ids) = counting_sort(
            jp_groups,
            (0..n).map(|e| Some(jp_of_group[group[e] as usize])),
        );
        let mut start = 0usize;
        for &end in &post_end {
            rare_last(&mut ids[start..end as usize], is_rare, &mut scratch);
            start = end as usize;
        }
        jianpin.post.end = post_end.into();
        jianpin.post.ids = ids.into();
        jianpin.table = IdTable::with_capacity(jp_groups);
        let mut initial_prefixes = FpSet::default();
        for jp in 0..jp_groups as u32 {
            let key = jianpin.key(jp);
            let mut hash = KEY_HASH_SEED;
            for &letter in &key[..key.len() - 1] {
                hash = initial_hash_step(hash, letter);
                initial_prefixes.insert(hash);
            }
            let hash = initials_hash(key);
            jianpin.table.insert(hash, jp, |_| unreachable!("容量够"));
        }
        initial_prefixes.compact();

        let mut lex = MemoryLexicon {
            words: p.words.into(),
            word_end: p.word_end.into(),
            freqs: p.freqs.into(),
            group: group.into(),
            full,
            jianpin,
            fuzzy_index: OnceLock::new(),
            full_prefixes,
            initial_prefixes,
            total_freq: p.total_freq,
            skipped: p.skipped,
            blob: None,
        };
        // 一边解析一边长的数组留着一半空位，收紧
        lex.words.shrink();
        lex.word_end.shrink();
        lex.freqs.shrink();
        lex.full.keys.shrink();
        lex.full.key_end.shrink();
        lex.full.post.end.shrink();
        lex.jianpin.keys.shrink();
        lex.jianpin.key_end.shrink();
        Ok(lex)
    }

    /// 从 TSV 加载，并在旁边维护一份二进制快照。
    ///
    /// 55 万词库的 TSV 解析要 1.3 秒，快照能把这部分省掉。快照按
    /// 「源文件大小 + mtime + 格式版本」校验，词库更新后自动重建。
    pub fn from_tsv_path_cached<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let Some((len, mtime)) = source_stamp(path) else {
            return Self::from_tsv_path(path);
        };
        let candidates = cache_paths(path);
        for cache in &candidates {
            if let Ok(blob) = Blob::open(cache) {
                if let Some(lex) = Self::read_blob(Arc::new(blob), len, mtime) {
                    return Ok(lex);
                }
            }
        }
        let lex = Self::from_tsv_path(path)?;
        // 写失败不算错误：只读目录会退到 XDG 缓存目录，那里也不行就算了
        for cache in &candidates {
            if lex.write_cache(cache, len, mtime).is_ok() {
                break;
            }
        }
        Ok(lex)
    }

    /// 快照里一节一节的数组（顺序就是文件里的顺序）。
    fn sections(&self) -> Vec<&[u8]> {
        fn bytes<T: Copy>(a: &[T]) -> &[u8] {
            // 安全：u8/u16/u32/f64 没有填充字节
            unsafe { std::slice::from_raw_parts(a.as_ptr().cast::<u8>(), std::mem::size_of_val(a)) }
        }
        let fz = self.fuzzy();
        vec![
            bytes(&self.words[..]),
            bytes(&self.word_end[..]),
            bytes(&self.freqs[..]),
            bytes(&self.group[..]),
            bytes(&self.full.key_end[..]),
            bytes(&self.full.keys[..]),
            bytes(&self.full.post.end[..]),
            bytes(&self.full.post.ids[..]),
            bytes(&self.full.table.slots[..]),
            bytes(&self.jianpin.key_end[..]),
            bytes(&self.jianpin.keys[..]),
            bytes(&self.jianpin.post.end[..]),
            bytes(&self.jianpin.post.ids[..]),
            bytes(&self.jianpin.table.slots[..]),
            bytes(&self.full_prefixes.slots[..]),
            bytes(&self.initial_prefixes.slots[..]),
            bytes(&fz.rep[..]),
            bytes(&fz.post.end[..]),
            bytes(&fz.post.ids[..]),
            bytes(&fz.table.slots[..]),
        ]
    }

    /// 快照：头（魔数、版本、源文件大小和 mtime、总频次、校验和、各节的位置），后面是各节原样的数组，
    /// 每节 8 字节对齐。只在小端机器上写（读的时候直接当数组用）。
    fn write_cache(&self, cache: &Path, src_len: u64, src_mtime: u64) -> Result<()> {
        if cfg!(target_endian = "big") {
            bail!("大端机器不写快照");
        }
        if !self.full.post.extra.is_empty() || !self.jianpin.post.extra.is_empty() {
            bail!("建好以后又加过词的词典不写快照");
        }
        let sections = self.sections();
        let header = CACHE_HEADER + sections.len() * 16;
        let mut offsets = Vec::with_capacity(sections.len());
        let mut at = header;
        for s in &sections {
            offsets.push(at);
            at = (at + s.len()).div_ceil(8) * 8;
        }
        let mut buf: Vec<u8> = vec![0; at];
        for (s, &off) in sections.iter().zip(&offsets) {
            buf[off..off + s.len()].copy_from_slice(s);
        }
        buf[0..4].copy_from_slice(CACHE_MAGIC);
        buf[4..8].copy_from_slice(&cache_version().to_le_bytes());
        buf[8..16].copy_from_slice(&src_len.to_le_bytes());
        buf[16..24].copy_from_slice(&src_mtime.to_le_bytes());
        // 词频是估过的（见 estimate_placeholder_freqs），总频次单独存，别从估值里加
        buf[24..32].copy_from_slice(&self.total_freq.to_le_bytes());
        buf[32..36].copy_from_slice(&(sections.len() as u32).to_le_bytes());
        for (i, (s, &off)) in sections.iter().zip(&offsets).enumerate() {
            let at = CACHE_HEADER + i * 16;
            buf[at..at + 8].copy_from_slice(&(off as u64).to_le_bytes());
            buf[at + 8..at + 16].copy_from_slice(&(s.len() as u64).to_le_bytes());
        }
        let sum = crate::lexstore::checksum(&buf[CACHE_HEADER..]);
        buf[40..48].copy_from_slice(&sum.to_le_bytes());
        if let Some(parent) = cache.parent() {
            fs::create_dir_all(parent).ok();
        }
        crate::fsutil::write_atomic(cache, &buf)
            .with_context(|| format!("写词库快照失败: {}", cache.display()))
    }

    /// 读一份快照的字节（测试用；真的载入走 [`MemoryLexicon::read_blob`]，不复制）。
    #[cfg(test)]
    fn read_cache(data: &[u8], src_len: u64, src_mtime: u64) -> Option<MemoryLexicon> {
        Self::read_blob(Arc::new(Blob::from_bytes(data)), src_len, src_mtime)
    }

    /// 从快照建词典：各列直接指着 `blob`。版本、源文件、校验和对不上，或者哪一节的结构不对
    /// （偏移越界、下标超出、不是合法 UTF-8……）都返回 None，调用方退回去读 TSV。
    /// 检查过的东西查词时不会越界、不会死循环。
    fn read_blob(blob: Arc<Blob>, src_len: u64, src_mtime: u64) -> Option<MemoryLexicon> {
        if cfg!(target_endian = "big") {
            return None;
        }
        let data = blob.bytes();
        let u32_at = |at: usize| Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?));
        let u64_at = |at: usize| Some(u64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?));
        if data.get(0..4)? != CACHE_MAGIC || u32_at(4)? != cache_version() {
            return None;
        }
        if u64_at(8)? != src_len || u64_at(16)? != src_mtime {
            return None;
        }
        let total_freq = f64::from_bits(u64_at(24)?);
        let count = u32_at(32)? as usize;
        if count != CACHE_SECTIONS {
            return None;
        }
        if crate::lexstore::checksum(data.get(CACHE_HEADER..)?) != u64_at(40)? {
            return None;
        }
        let mut next = 0usize;
        let mut section = |size: usize| -> Option<(usize, usize)> {
            let at = CACHE_HEADER + next * 16;
            next += 1;
            let off = usize::try_from(u64_at(at)?).ok()?;
            let len = usize::try_from(u64_at(at + 8)?).ok()?;
            (len % size == 0).then_some((off, len / size))
        };
        macro_rules! arr {
            ($t:ty) => {{
                let (off, n) = section(std::mem::size_of::<$t>())?;
                blob.section::<$t>(off, n)?
            }};
        }
        let words: Arr<u8> = arr!(u8);
        let word_end: Arr<u32> = arr!(u32);
        let freqs: Arr<f64> = arr!(f64);
        let group: Arr<u32> = arr!(u32);
        let mut full = Groups::<u16> {
            key_end: arr!(u32),
            keys: arr!(u16),
            ..Groups::default()
        };
        full.post.end = arr!(u32);
        full.post.ids = arr!(u32);
        let full_slots: Arr<u32> = arr!(u32);
        let mut jianpin = Groups::<u8> {
            key_end: arr!(u32),
            keys: arr!(u8),
            ..Groups::default()
        };
        jianpin.post.end = arr!(u32);
        jianpin.post.ids = arr!(u32);
        let jp_slots: Arr<u32> = arr!(u32);
        let full_prefixes = FpSet::from_slots(arr!(u32))?;
        let initial_prefixes = FpSet::from_slots(arr!(u32))?;
        let mut fuzzy = FuzzyIndex {
            rep: arr!(u32),
            ..FuzzyIndex::default()
        };
        fuzzy.post.end = arr!(u32);
        fuzzy.post.ids = arr!(u32);
        let fz_slots: Arr<u32> = arr!(u32);

        // 词条：字的偏移单调，每个词不空、是合法 UTF-8；读音组号在范围内
        let n = freqs.len();
        if word_end.len() != n || group.len() != n {
            return None;
        }
        // 整块是合法 UTF-8、每个词的结尾都落在字的边界上，每个词就都是合法 UTF-8
        std::str::from_utf8(&words).ok()?;
        if !crate::lexstore::strictly_increasing(&word_end, words.len())
            || word_end
                .iter()
                .any(|&end| words.get(end as usize).is_some_and(|&b| (b as i8) < -0x40))
        {
            return None;
        }
        let groups = full.len();
        if !full.check_keys()
            || full
                .keys
                .iter()
                .any(|&raw| usize::from(raw & 0x7fff) >= crate::syllable::SYLLABLES.len())
            || !crate::lexstore::all_below(&group, groups)
            || !full.post.check(groups, n)
        {
            return None;
        }
        full.table = IdTable::from_slots(full_slots, groups)?;
        let jp_groups = jianpin.len();
        if !jianpin.check_keys() || !jianpin.post.check(jp_groups, n) {
            return None;
        }
        jianpin.table = IdTable::from_slots(jp_slots, jp_groups)?;
        let fz_groups = fuzzy.rep.len();
        if !crate::lexstore::all_below(&fuzzy.rep, groups) || !fuzzy.post.check(fz_groups, n) {
            return None;
        }
        fuzzy.table = IdTable::from_slots(fz_slots, fz_groups)?;

        Some(MemoryLexicon {
            words,
            word_end,
            freqs,
            group,
            full,
            jianpin,
            fuzzy_index: OnceLock::from(fuzzy),
            full_prefixes,
            initial_prefixes,
            total_freq,
            skipped: 0,
            blob: Some(blob),
        })
    }

    /// 从 TSV 文本加载。
    pub fn from_tsv_str(data: &str) -> Result<Self> {
        let mut parsed = Parsed::default();
        for (line_no, raw) in data.lines().enumerate() {
            let line = raw.trim_end();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut cols = line.split('\t');
            let pinyin = cols.next().unwrap_or("");
            let word = cols.next().unwrap_or("");
            let freq = cols.next().unwrap_or("");
            if cols.next().is_some() || pinyin.is_empty() || word.is_empty() || freq.is_empty() {
                bail!("第 {} 行应为 3 列：拼音<TAB>词<TAB>频次", line_no + 1);
            }
            let freq: f64 = freq
                .parse()
                .with_context(|| format!("第 {} 行频次不是数字: {freq}", line_no + 1))?;
            // 音节不认识（各地方言音、冷僻拼音）时跳过这一行，
            // 不要让一条脏数据毁掉整个词库。
            match check_entry(pinyin, word, freq) {
                Ok(keys) => {
                    parsed.words.extend_from_slice(word.as_bytes());
                    parsed.word_end.push(parsed.words.len() as u32);
                    parsed.freqs.push(freq);
                    parsed.keys.extend(keys.iter().map(|&k| raw_key(k)));
                    parsed.key_end.push(parsed.keys.len() as u32);
                    parsed.total_freq += freq;
                }
                Err(_) => parsed.skipped += 1,
            }
        }
        Self::build(parsed)
    }

    /// 给「占位频次」的词估一个频次。
    ///
    /// rime-ice 的 ext / tencent 两个词表（加起来五十多万词，占整个词库一半以上）
    /// 所有词的频次都是同一个占位值 100。按字面当频次用，这些词就比任何常用字
    /// 拼出来的句子都冷门得多：`bujianyi` 里「不建议」排在「不见一」「部件」后面，
    /// 第一页根本看不到。
    ///
    /// 做法：找出多音节词里占比很高的那个频次（占位值），把这些词按自己的拼音
    /// 和字切成词库里已有的词，频次取「各部分概率相乘」再乘一个成词加成
    /// [`PLACEHOLDER_BONUS`]——词库收了它，说明这几个字常挨着用，相当于没有
    /// 二元模型时给它补上搭配分。不会低于原值，也不会高过最冷门的那一部分。
    fn estimate_placeholder_freqs(&mut self, min_words: usize) {
        let n = self.len() as u32;
        let mut hist: HashMap<u64, usize> = HashMap::new();
        let mut multi = 0usize;
        for i in 0..n {
            if self.keys_at(i).len() >= 2 {
                multi += 1;
                *hist.entry(self.freq(i).to_bits()).or_default() += 1;
            }
        }
        let Some((&bits, &count)) = hist.iter().max_by_key(|(_, &c)| c) else {
            return;
        };
        if count < min_words || count * 4 < multi {
            return;
        }
        let placeholder = f64::from_bits(bits);
        let bonus = std::env::var("ARCHTYPE_PLACEHOLDER_BONUS")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(PLACEHOLDER_BONUS);
        let total = self.total_freq.max(1.0);
        let mut updates = Vec::new();
        for idx in 0..n {
            let keys = self.keys_at(idx);
            let entry_freq = self.freq(idx);
            if entry_freq != placeholder || keys.len() < 2 {
                continue;
            }
            let chars: Vec<char> = self.word(idx).chars().collect();
            let n = chars.len();
            if n != keys.len() {
                continue; // 儿化、混了字母数字的词，字和音节对不齐
            }
            // best[j]：前 j 个字切成已有词的最小代价；rarest[j]：这种切法里最冷门部分的频次
            let mut best = vec![f64::INFINITY; n + 1];
            let mut rarest = vec![f64::INFINITY; n + 1];
            best[0] = 0.0;
            for i in 0..n {
                if !best[i].is_finite() {
                    continue;
                }
                for j in (i + 1)..=n.min(i + PLACEHOLDER_MAX_PART) {
                    if j - i == n {
                        continue; // 不能用自己
                    }
                    let Some(g) = self.full.find(raw_hash(&keys[i..j]), &keys[i..j]) else {
                        continue;
                    };
                    let part: String = chars[i..j].iter().collect();
                    let freq = self
                        .full
                        .post
                        .frozen(g)
                        .iter()
                        .chain(self.full.post.added(g))
                        .filter(|&&k| self.word(k) == part)
                        .map(|&k| self.freq(k))
                        .fold(0.0f64, f64::max);
                    if freq <= 0.0 {
                        continue;
                    }
                    let cost = best[i] - (freq / total).ln();
                    if cost < best[j] {
                        best[j] = cost;
                        rarest[j] = rarest[i].min(freq);
                    }
                }
            }
            if !best[n].is_finite() {
                continue;
            }
            let estimate = (total * (bonus - best[n]).exp()).min(rarest[n]);
            if estimate > entry_freq {
                updates.push((idx, estimate));
            }
        }
        let freqs = self.freqs.to_mut();
        for (idx, freq) in updates {
            freqs[idx as usize] = freq;
        }
    }

    fn build_fuzzy_index(&self) -> FuzzyIndex {
        let groups = self.full.len();
        // 每个全拼组的归并键；有音节没有归并键的组不进索引
        let mut fk_end = Vec::with_capacity(groups);
        let mut fk: Vec<u32> = Vec::with_capacity(self.full.keys.len());
        let mut valid = vec![true; groups];
        for g in 0..groups as u32 {
            let before = fk.len();
            for &raw in self.full.key(g) {
                match crate::syllable::fuzzy_key(raw & 0x7fff) {
                    Some(k) => fk.push(k),
                    None => {
                        fk.truncate(before);
                        valid[g as usize] = false;
                        break;
                    }
                }
            }
            fk_end.push(fk.len() as u32);
        }
        let fkey = |g: u32| {
            let g = g as usize;
            let start = if g == 0 { 0 } else { fk_end[g - 1] as usize };
            &fk[start..fk_end[g] as usize]
        };
        let mut gorder: Vec<u32> = (0..groups as u32).filter(|&g| valid[g as usize]).collect();
        gorder.sort_unstable_by(|&a, &b| fkey(a).cmp(fkey(b)).then(a.cmp(&b)));
        let mut rep = Vec::new();
        let mut fz_of_group = vec![u32::MAX; groups];
        for (pos, &g) in gorder.iter().enumerate() {
            if pos == 0 || fkey(gorder[pos - 1]) != fkey(g) {
                rep.push(g);
            }
            fz_of_group[g as usize] = rep.len() as u32 - 1;
        }
        drop(gorder);
        let fz_groups = rep.len();
        // 组里的词条按下标（加词的先后），和以前逐条往 HashMap 里加的顺序一样
        let (post_end, ids) = counting_sort(
            fz_groups,
            self.group.iter().map(|&g| {
                let f = fz_of_group[g as usize];
                (f != u32::MAX).then_some(f)
            }),
        );
        let mut table = IdTable::with_capacity(fz_groups);
        for (f, &g) in rep.iter().enumerate() {
            table.insert(fuzzy_hash(fkey(g).iter().copied()), f as u32, |_| {
                unreachable!("容量够")
            });
        }
        FuzzyIndex {
            rep: rep.into(),
            post: Postings {
                end: post_end.into(),
                ids: ids.into(),
                extra: HashMap::new(),
            },
            table,
        }
    }

    fn fuzzy(&self) -> &FuzzyIndex {
        self.fuzzy_index.get_or_init(|| self.build_fuzzy_index())
    }

    /// 加载时被跳过的行数。
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    /// 占的内存：(堆, 快照)，字节。快照在 Unix 上是映射进来的文件，碰到过的页才算常驻内存。
    pub fn memory_usage(&self) -> (usize, usize) {
        let fz = self.fuzzy_index.get();
        let heap = self.words.heap_bytes()
            + self.word_end.heap_bytes()
            + self.freqs.heap_bytes()
            + self.group.heap_bytes()
            + self.full.heap_bytes()
            + self.jianpin.heap_bytes()
            + self.full_prefixes.heap_bytes()
            + self.initial_prefixes.heap_bytes()
            + fz.map_or(0, |f| {
                f.rep.heap_bytes() + f.post.heap_bytes() + f.table.heap_bytes()
            });
        (heap, self.blob.as_ref().map_or(0, |b| b.bytes().len()))
    }

    /// 从 TSV 文件加载。
    pub fn from_tsv_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let data = fs::read_to_string(path)
            .with_context(|| format!("读取词典失败: {}", path.display()))?;
        let mut lex = Self::from_tsv_str(&data)?;
        drop(data);
        lex.estimate_placeholder_freqs(PLACEHOLDER_MIN_WORDS);
        Ok(lex)
    }
}

/// 计数排序：`keys` 按顺序给每个元素的组号（None 不要），返回每组的结束位置和按组排好的元素下标
/// （同组里还是原来的先后）。
fn counting_sort(
    groups: usize,
    keys: impl Iterator<Item = Option<u32>> + Clone,
) -> (Vec<u32>, Vec<u32>) {
    let mut end = vec![0u32; groups];
    for g in keys.clone().flatten() {
        end[g as usize] += 1;
    }
    let mut sum = 0u32;
    for e in end.iter_mut() {
        sum += *e;
        *e = sum;
    }
    let mut fill: Vec<u32> = std::iter::once(0)
        .chain(end.iter().copied())
        .take(groups)
        .collect();
    let mut ids = vec![0u32; sum as usize];
    for (i, g) in keys.enumerate() {
        if let Some(g) = g {
            let at = &mut fill[g as usize];
            ids[*at as usize] = i as u32;
            *at += 1;
        }
    }
    (end, ids)
}

/// 只有一个字。
#[inline]
fn is_single_char(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some() && chars.next().is_none()
}

impl MemoryLexicon {
    /// 第 g 组（`post` 里的）词条按顺序交给 `f`：生僻字排在每组最后，`skip_rare` 时不要它们。
    /// 建好以后加进来的（`extra`）接在建好时那一段的后面，生僻字照样排在所有不是生僻字的后面。
    #[inline]
    fn each_in(&self, post: &Postings, g: u32, skip_rare: bool, f: &mut impl FnMut(u32)) {
        let frozen = post.frozen(g);
        let added = post.added(g);
        if added.is_empty() {
            let list = if skip_rare {
                &frozen[..frozen.partition_point(|&i| !self.is_rare_at(i))]
            } else {
                frozen
            };
            list.iter().for_each(|&i| f(i));
            return;
        }
        let a = frozen.partition_point(|&i| !self.is_rare_at(i));
        let b = added.partition_point(|&i| !self.is_rare_at(i));
        frozen[..a].iter().chain(&added[..b]).for_each(|&i| f(i));
        if !skip_rare {
            frozen[a..].iter().chain(&added[b..]).for_each(|&i| f(i));
        }
    }

    fn lookup_impl(&self, query: &[SyllableQuery], skip_rare: bool) -> Vec<LexEntry> {
        let mut found = Vec::new();
        self.visit_ids(query, skip_rare, &mut |i| found.push(self.entry(i)));
        found
    }

    /// 查词，命中的词条下标按顺序交给 `f`。lookup、visit 都走这里，两种查法结果一样。
    fn visit_ids(&self, query: &[SyllableQuery], skip_rare: bool, f: &mut impl FnMut(u32)) {
        if query.is_empty() {
            return;
        }

        let all_full = query
            .iter()
            .all(|q| matches!(q, SyllableQuery::Full { .. }));
        let all_jianpin = query
            .iter()
            .all(|q| matches!(q, SyllableQuery::Jianpin { .. }));

        if all_full {
            let mut hash = KEY_HASH_SEED;
            let mut erhua = false;
            for q in query {
                if let SyllableQuery::Full { id, erhua: e } = q {
                    hash = key_hash_step(hash, SyllableKey::new(*id, *e));
                    erhua |= *e;
                }
            }
            let found = self.full.table.find(hash, |g| {
                let key = self.full.key(g);
                key.len() == query.len()
                    && key.iter().zip(query).all(|(&raw, q)| match q {
                        SyllableQuery::Full { id, erhua } => {
                            raw == raw_key(SyllableKey::new(*id, *erhua))
                        }
                        SyllableQuery::Jianpin { .. } => false,
                    })
            });
            if let Some(g) = found {
                self.each_in(&self.full.post, g, skip_rare, f);
            }
            // 儿化（wanr、yidianr）：词库里儿化词都写成带 er 音节的（wan er 玩儿），
            // 没有 `wan r` 这种写法，所以带儿化标记的音节再按「音节 + er」查一遍
            if erhua {
                let key: Vec<SyllableKey> = query
                    .iter()
                    .map(|q| match q {
                        SyllableQuery::Full { id, erhua } => SyllableKey::new(*id, *erhua),
                        _ => unreachable!(),
                    })
                    .collect();
                if let Some(spelled) = erhua_spelled_out(&key) {
                    let raw: Vec<u16> = spelled.iter().map(|&k| raw_key(k)).collect();
                    if let Some(g) = self.full.find(raw_hash(&raw), &raw) {
                        self.each_in(&self.full.post, g, skip_rare, &mut |i| {
                            if self.word(i).contains('儿') {
                                f(i);
                            }
                        });
                    }
                }
            }
            return;
        }

        if all_jianpin && !query.iter().any(needs_initial_check) {
            let key: Vec<u8> = query
                .iter()
                .map(|q| match q {
                    SyllableQuery::Jianpin { letter } => *letter,
                    _ => unreachable!(),
                })
                .collect();
            if let Some(jp) = self.jianpin.find(initials_hash(&key), &key) {
                self.each_in(&self.jianpin.post, jp, skip_rare, f);
            }
            return;
        }

        // 全拼混简拼（k yi -> 可以）、zh/ch/sh 简拼：先按首字母查，再逐个比对
        let Some(initials) = query_initials(query) else {
            return;
        };
        if let Some(jp) = self.jianpin.find(initials_hash(&initials), &initials) {
            self.each_in(&self.jianpin.post, jp, skip_rare, &mut |i| {
                if keys_match(self.keys_at(i), query) {
                    f(i);
                }
            });
        }
    }

    /// 全部词条（测试用）。
    #[cfg(test)]
    fn all_entries(&self) -> Vec<LexEntry> {
        (0..self.len() as u32).map(|i| self.entry(i)).collect()
    }
}

impl Lexicon for MemoryLexicon {
    fn lookup(&self, query: &[SyllableQuery]) -> Vec<LexEntry> {
        self.lookup_impl(query, false)
    }

    fn lookup_common(&self, query: &[SyllableQuery]) -> Vec<LexEntry> {
        self.lookup_impl(query, true)
    }

    fn visit(&self, query: &[SyllableQuery], common: bool, f: &mut dyn FnMut(&str, f64)) {
        self.visit_ids(query, common, &mut |i| f(self.word(i), self.freq(i)));
    }

    fn has_longer(&self, query: &[SyllableQuery]) -> bool {
        if query.iter().any(|q| q.is_jianpin()) {
            // 简拼、混合查询按声母剪（只看声母会少剪一些，不会错剪）
            let Some(initials) = query_initials(query) else {
                return true;
            };
            return self.initial_prefixes.contains(initials_hash(&initials));
        }
        let mut hash = KEY_HASH_SEED;
        let mut erhua = false;
        for q in query {
            if let SyllableQuery::Full { id, erhua: e } = q {
                hash = key_hash_step(hash, SyllableKey::new(*id, *e));
                erhua |= *e;
            }
        }
        if self.full_prefixes.contains(hash) {
            return true;
        }
        if !erhua {
            return false;
        }
        // 儿化在词中间的（wanrming -> 玩儿命）：按「音节 + er」的写法再看一次
        let key: Vec<SyllableKey> = query
            .iter()
            .filter_map(|q| match q {
                SyllableQuery::Full { id, erhua } => Some(SyllableKey::new(*id, *erhua)),
                SyllableQuery::Jianpin { .. } => None,
            })
            .collect();
        erhua_spelled_out(&key).is_some_and(|spelled| {
            let hash = spelled
                .iter()
                .fold(KEY_HASH_SEED, |h, k| key_hash_step(h, *k));
            self.full_prefixes.contains(hash)
        })
    }

    fn prepare_fuzzy(&self) {
        // 93 万条的词库约 0.1 秒；从快照载入的已经带着
        self.fuzzy();
    }

    fn lookup_fuzzy(&self, query: &[SyllableQuery], groups: u32) -> Vec<LexEntry> {
        if query.is_empty() || groups == 0 {
            return Vec::new();
        }
        let mut ids = Vec::with_capacity(query.len());
        let mut key = Vec::with_capacity(query.len());
        for q in query {
            match q {
                SyllableQuery::Full { id, erhua } => {
                    if *erhua {
                        return Vec::new();
                    }
                    match crate::syllable::fuzzy_key(*id) {
                        Some(canon) => key.push(canon),
                        None => return Vec::new(),
                    }
                    ids.push(*id);
                }
                SyllableQuery::Jianpin { .. } => return Vec::new(),
            }
        }
        let index = self.fuzzy();
        let found = index.table.find(fuzzy_hash(key.iter().copied()), |f| {
            let rep = self.full.key(index.rep[f as usize]);
            rep.len() == key.len()
                && rep
                    .iter()
                    .zip(&key)
                    .all(|(&raw, &k)| crate::syllable::fuzzy_key(raw & 0x7fff) == Some(k))
        });
        let Some(f) = found else {
            return Vec::new();
        };
        // 索引按全部分组归并，这里只留下「用到的分组都打开了」的词
        index
            .post
            .frozen(f)
            .iter()
            .filter(|&&i| {
                let keys = self.keys_at(i);
                keys.len() == ids.len()
                    && keys.iter().zip(&ids).all(|(&raw, &q)| {
                        crate::syllable::fuzzy_groups_between(q, raw & 0x7fff)
                            .is_some_and(|need| need & !groups == 0)
                    })
            })
            .map(|&i| self.entry(i))
            .collect()
    }

    fn total_freq(&self) -> f64 {
        self.total_freq
    }

    fn len(&self) -> usize {
        self.freqs.len()
    }
}

/// 给束搜索用的视图：精确读音之外，再加上模糊音读音的词。
///
/// 以前只在精确读音一个词都没有时才退到模糊音，可 zong、si、dan si 这些本身就有词
/// （总、四、单丝），模糊音永远轮不到，打不出中、是、但是（#93）。
/// 现在模糊音读音和精确读音一起参与组句、排序，只是每个模糊词的代价多 `penalty`
/// （自然对数），差不多常用时精确读音在前。
pub struct FuzzyLexicon<'a> {
    inner: &'a dyn Lexicon,
    groups: u32,
    penalty: f64,
}

impl<'a> FuzzyLexicon<'a> {
    /// groups：打开的模糊音分组（`syllable::fuzzy::*` 位掩码）；penalty：模糊词多出的代价。
    pub fn new(inner: &'a dyn Lexicon, groups: u32, penalty: f64) -> Self {
        Self {
            inner,
            groups,
            penalty,
        }
    }

    /// 精确结果后面接上只有模糊音才对得上的词：每用一处模糊音代价多 penalty（频次打折）。
    fn with_fuzzy(
        &self,
        query: &[SyllableQuery],
        mut found: Vec<LexEntry>,
        common: bool,
    ) -> Vec<LexEntry> {
        let penalty = self.penalty.max(0.0);
        for mut entry in self.inner.lookup_fuzzy(query, self.groups) {
            if common && entry.is_rare() {
                continue;
            }
            // 用了几处模糊音：san -> shang 是声母、韵母各一处
            let used: u32 = entry
                .keys
                .iter()
                .zip(query)
                .map(|(k, q)| match q {
                    SyllableQuery::Full { id, .. } => {
                        crate::syllable::fuzzy_groups_between(*id, k.id).map_or(0, u32::count_ones)
                    }
                    SyllableQuery::Jianpin { .. } => 0,
                })
                .sum();
            // lookup_fuzzy 也返回读音完全相同的词，精确那份已经有了
            if used == 0 {
                continue;
            }
            entry.freq *= (-penalty * f64::from(used)).exp();
            found.push(entry);
        }
        found
    }
}

impl Lexicon for FuzzyLexicon<'_> {
    fn lookup(&self, query: &[SyllableQuery]) -> Vec<LexEntry> {
        self.with_fuzzy(query, self.inner.lookup(query), false)
    }

    fn lookup_common(&self, query: &[SyllableQuery]) -> Vec<LexEntry> {
        self.with_fuzzy(query, self.inner.lookup_common(query), true)
    }

    fn lookup_fuzzy(&self, query: &[SyllableQuery], groups: u32) -> Vec<LexEntry> {
        self.inner.lookup_fuzzy(query, groups)
    }

    fn total_freq(&self) -> f64 {
        self.inner.total_freq()
    }

    fn len(&self) -> usize {
        self.inner.len()
    }
}

/// 给用户词库就地改动用的两个入口。查询那边不认识它们：改完以后的样子必须和
/// 「把同样的词按同样的顺序从头 insert 一遍」完全一样（`user_lexicon_matches_fresh_build` 测这个）。
impl MemoryLexicon {
    /// 接在最后加一个词，返回它的词条下标。
    fn append(&mut self, pinyin: &str, word: &str, freq: f64) -> Result<usize> {
        // insert 会丢掉模糊音索引（多了一个词就不全了），下次模糊查询再建
        self.insert(pinyin, word, freq)?;
        Ok(self.len() - 1)
    }

    /// 只改一个词条的频次，索引不动。改不了（频次不合法、改完生僻字的身份变了——索引里
    /// 生僻字排在每组最后）返回 false，什么都没动，调用方整份重建。
    fn set_entry_freq(&mut self, idx: usize, freq: f64) -> bool {
        if !freq.is_finite() || freq < 0.0 || idx >= self.len() {
            return false;
        }
        let i = idx as u32;
        let was_rare = self.is_rare_at(i);
        let old = std::mem::replace(&mut self.freqs.to_mut()[idx], freq);
        if self.is_rare_at(i) != was_rare {
            self.freqs.to_mut()[idx] = old;
            return false;
        }
        // 从头加一遍而不是加上差值：和重建出来的一位不差，改多少次也不会越漂越远。
        // 几千个数相加是几微秒
        self.total_freq = self.freqs.iter().fold(0.0, |sum, f| sum + f);
        true
    }
}

/// 用户词（学到的、手动加的），连同给它们建的词典。两样放在一把锁里，改的时候不会对不上。
#[derive(Default)]
struct UserWords {
    /// (拼音, 词, 频次)，按加进来的先后；一个词只有一条。
    list: Vec<(String, String, f64)>,
    /// 词 -> `list` 里的下标。
    pos: HashMap<String, usize>,
    /// `list[i]` 在 `lexicon` 里的词条下标；没收进去的（频次不合法等）是 None。
    slot: Vec<Option<usize>>,
    lexicon: MemoryLexicon,
}

impl UserWords {
    /// 记下一个词（已有就换掉拼音和频次），不动词典。返回 `list` 里的下标、是不是新加的。
    fn put(&mut self, pinyin: &str, word: &str, freq: f64) -> (usize, bool) {
        if let Some(&i) = self.pos.get(word) {
            let item = &mut self.list[i];
            if item.0 != pinyin {
                item.0 = pinyin.to_string();
            }
            item.2 = freq;
            return (i, false);
        }
        let i = self.list.len();
        self.list.push((pinyin.to_string(), word.to_string(), freq));
        self.pos.insert(word.to_string(), i);
        self.slot.push(None);
        (i, true)
    }

    /// 词典从头建一遍。
    fn rebuild(&mut self) {
        let mut lexicon = MemoryLexicon::new();
        self.slot.clear();
        for (pinyin, word, freq) in &self.list {
            let slot = if storable(pinyin, word) {
                lexicon
                    .insert(pinyin, word, *freq)
                    .ok()
                    .map(|()| lexicon.len() - 1)
            } else {
                None
            };
            self.slot.push(slot);
        }
        self.lexicon = lexicon;
    }
}

/// 拼音、词里带制表符或换行的存不进 TSV（读回来就错位了），不收进词典。
/// 以前这样一个词会让整份用户词典重建失败、之后加的词全都不生效。
fn storable(pinyin: &str, word: &str) -> bool {
    !pinyin.contains(['\t', '\n']) && !word.contains(['\t', '\n'])
}

/// 用户词库：在基础词库之上叠加用户自己添加的词。
///
/// 用户词以 (拼音, 词, 频次) 保存，另外给它们建一份小的 MemoryLexicon。几千条用户词从头建
/// 一遍要几毫秒，所以能不重建就不重建：从文件载入是全部读完建一次；加新词接在最后；
/// 已有的词只是频次变了（学过的词每上屏一次就来一回，在按键的线程上）就地改。
/// 删词、改拼音才从头建。自定义词单独一份词典，换自定义词不用动用户词的。
pub struct UserLexicon {
    base: Arc<dyn Lexicon>,
    user: RwLock<UserWords>,
    /// 自定义词（设置里添加的，存在 custom.tsv）：只读，整份替换，不写回用户词库。
    custom: RwLock<Vec<(String, String, f64)>>,
    /// 自定义词里编码不是拼音的（mail -> 邮箱地址）：只在整个输入正好等于编码时给出。
    codes: RwLock<Vec<(String, String)>>,
    /// 全部自定义词按文件里的顺序（编码、词，词保留转义），给工具栏的「常用语」列表用。
    phrases: RwLock<Vec<(String, String)>>,
    /// `custom` 建成的词典。
    custom_lexicon: RwLock<MemoryLexicon>,
    /// 用户词的词典从头建过几次（测试看它，保证载入、改频次不会一个词重建一次）。
    rebuilds: AtomicUsize,
}

/// 用户词库存盘的内容（[`UserLexicon::save_tsv`]）：注释行 + 一行一个 `拼音<TAB>词<TAB>频次`。
pub fn user_dict_text(words: &[(String, String, f64)]) -> String {
    let mut out = String::from("# 用户词库\n");
    for (pinyin, word, freq) in words {
        out.push_str(&format!("{}\t{}\t{}\n", pinyin, word, *freq as i64));
    }
    out
}

/// 自定义词默认频次：和最常用的词差不多，所以排在同音词前面。
pub const CUSTOM_WORD_FREQ: f64 = 5_000_000.0;

impl UserLexicon {
    pub fn new(base: Arc<dyn Lexicon>) -> Self {
        Self {
            base,
            user: RwLock::new(UserWords::default()),
            custom: RwLock::new(Vec::new()),
            codes: RwLock::new(Vec::new()),
            phrases: RwLock::new(Vec::new()),
            custom_lexicon: RwLock::new(MemoryLexicon::new()),
            rebuilds: AtomicUsize::new(0),
        }
    }

    /// 拼音、词能不能当用户词：都不能空，拼音要解析得出来。返回去掉首尾空白的拼音。
    fn check_word<'a>(pinyin: &'a str, word: &str) -> Result<&'a str> {
        let pinyin = pinyin.trim();
        if word.is_empty() || pinyin.is_empty() {
            bail!("用户词的拼音和词都不能为空");
        }
        // 先单独校验一次，拼音非法时不要污染词库。
        // 注意不能借用 from_tsv_str：它对未知音节是「跳过」而不是报错。
        parse_pinyin_keys(pinyin).context("用户词拼音无法解析")?;
        Ok(pinyin)
    }

    /// 用户词的词典从头建一遍（调用方拿着写锁）。
    fn rebuild_user(&self, user: &mut UserWords) {
        user.rebuild();
        self.rebuilds.fetch_add(1, Ordering::Relaxed);
    }

    /// 用户词的词典从头建过几次。
    #[cfg(test)]
    fn rebuild_count(&self) -> usize {
        self.rebuilds.load(Ordering::Relaxed)
    }

    /// 添加或更新一个用户词。拼音可以是 ni hao / nihao / xi'an。
    pub fn add(&self, pinyin: &str, word: &str, freq: f64) -> Result<()> {
        let pinyin = Self::check_word(pinyin, word)?;
        let mut guard = write_lock(&self.user);
        let user = &mut *guard;
        let same_pinyin = user
            .pos
            .get(word)
            .is_some_and(|&i| user.list[i].0 == pinyin);
        let (i, new) = user.put(pinyin, word, freq);
        if new {
            // 新词接在最后，和重建出来的顺序一样
            if storable(pinyin, word) {
                user.slot[i] = user.lexicon.append(pinyin, word, freq).ok();
            }
            return Ok(());
        }
        // 已有的词：读音没变、只是频次变了就地改；别的情况（改了读音、原来没收进词典、
        // 新频次不合法）少见，从头建。写法不一样（ni hao / ni'hao）读音也可能一样，比音节
        let same_reading = |lexicon: &MemoryLexicon, idx: usize| {
            same_pinyin
                || (storable(pinyin, word)
                    && parse_pinyin_keys(pinyin)
                        .is_ok_and(|keys| keys == lexicon.entry(idx as u32).keys))
        };
        let in_place = user.slot[i].is_some_and(|idx| {
            same_reading(&user.lexicon, idx) && user.lexicon.set_entry_freq(idx, freq)
        });
        if !in_place {
            self.rebuild_user(user);
        }
        Ok(())
    }

    /// 删除用户词，返回是否删掉了东西。
    pub fn remove(&self, word: &str) -> bool {
        let mut guard = write_lock(&self.user);
        let user = &mut *guard;
        let Some(at) = user.pos.remove(word) else {
            return false;
        };
        user.list.remove(at);
        for i in user.pos.values_mut() {
            if *i > at {
                *i -= 1;
            }
        }
        // 词典里的词条下标都跟着变，删词又少见（候选框里 Ctrl+Delete）：从头建
        self.rebuild_user(user);
        true
    }

    pub fn words(&self) -> Vec<(String, String, f64)> {
        self.user.read().map(|u| u.list.clone()).unwrap_or_default()
    }

    /// 用户词 word 的（拼音, 频次），不是用户词是 None。
    pub fn entry(&self, word: &str) -> Option<(String, f64)> {
        let user = self.user.read().ok()?;
        user.list
            .iter()
            .find(|(_, w, _)| w == word)
            .map(|(pinyin, _, freq)| (pinyin.clone(), *freq))
    }

    pub fn clear(&self) {
        if let Ok(mut user) = self.user.write() {
            *user = UserWords::default();
        }
    }

    /// 把用户词写成 TSV（与 lexicon 格式一致）。空的也写（只有注释行）：
    /// 用户把学到的词都删掉以后，旧文件不能留着，不然重启又回来了。
    pub fn save_tsv<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // 锁中毒时不能走 words()（它退化空表）：直接落盘会把用户词库清成只剩注释
        let words = self
            .user
            .read()
            .map(|u| u.list.clone())
            .map_err(|_| anyhow::anyhow!("用户词库锁已损坏，拒绝写回"))?;
        let out = user_dict_text(&words);
        // 输入法进程随时可能被系统杀掉：写一半的用户词库会把学到的词全弄丢
        crate::fsutil::write_atomic(path.as_ref(), out.as_bytes())
            .with_context(|| format!("写入用户词库失败: {}", path.as_ref().display()))
    }

    /// 从 TSV 载入用户词，追加到现有内容。
    ///
    /// 全部读完才建一次词典：以前一行调一次 [`UserLexicon::add`]、每次都整份重建，
    /// 3000 个词要 7 秒、6000 个 27 秒。
    ///
    /// 坏行（不是合法 UTF-8、拼音解析不了）跳过，别的行照常载入，见
    /// [`crate::fsutil::read_lines_lossy`]。
    pub fn load_tsv<P: AsRef<Path>>(&self, path: P) -> Result<usize> {
        let (lines, _bad) = crate::fsutil::read_lines_lossy(path.as_ref())
            .with_context(|| format!("读取用户词库失败: {}", path.as_ref().display()))?;
        // 解析、校验拼音不用拿锁；拿到锁以后只是记下来、建一次词典
        let mut words = Vec::new();
        for line in &lines {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let mut cols = line.split('\t');
            let (Some(pinyin), Some(word)) = (cols.next(), cols.next()) else {
                continue;
            };
            let freq = cols.next().and_then(|f| f.parse().ok()).unwrap_or(1000.0);
            if let Ok(pinyin) = Self::check_word(pinyin, word) {
                words.push((pinyin, word, freq));
            }
        }
        if !words.is_empty() {
            let mut user = write_lock(&self.user);
            for (pinyin, word, freq) in &words {
                user.put(pinyin, word, *freq);
            }
            self.rebuild_user(&mut user);
        }
        Ok(words.len())
    }

    /// 用一份自定义词整体替换现有的。格式 `编码<TAB>词[<TAB>频次]`。
    ///
    /// 编码一般是拼音（可以不分音节）；不是拼音、但全是字母的（mail、wdyx）当成自定义编码，
    /// 只在整个输入正好等于它时给出。词里可以写 `\n`（换行）、`\t`（制表符），上屏时才换成真的。
    /// 其他行跳过。返回收下的条数。
    pub fn set_custom_tsv(&self, data: &str) -> usize {
        let mut custom = Vec::new();
        let mut codes = Vec::new();
        let mut phrases = Vec::new();
        for line in data.lines() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let mut cols = line.split('\t');
            let (Some(pinyin), Some(word)) = (cols.next(), cols.next()) else {
                continue;
            };
            let (pinyin, word) = (pinyin.trim().to_ascii_lowercase(), word.trim());
            // 手写的 custom.tsv 可能不分音节（nihao）：自己切开
            if word.is_empty() {
                continue;
            }
            let code = pinyin.clone();
            let Some(pinyin) = (if parse_pinyin_keys(&pinyin).is_ok() {
                Some(pinyin)
            } else {
                split_syllables(&pinyin)
            }) else {
                let code: String = code.chars().filter(|c| *c != '\'').collect();
                if !code.is_empty() && code.bytes().all(|b| b.is_ascii_lowercase()) {
                    phrases.push((code.clone(), word.to_string()));
                    codes.push((code, word.to_string()));
                }
                continue;
            };
            phrases.push((code, word.to_string()));
            let freq = cols
                .next()
                .and_then(|f| f.trim().parse::<f64>().ok())
                .filter(|f| f.is_finite() && *f > 0.0)
                .unwrap_or(CUSTOM_WORD_FREQ);
            custom.push((pinyin, word.to_string(), freq));
        }
        let count = custom.len() + codes.len();
        // 只建自定义词这一份，用户词的词典不用动（用刚解析出来的 custom，不再回头读锁）
        let mut lexicon = MemoryLexicon::new();
        for (pinyin, word, freq) in &custom {
            let _ = lexicon.insert(pinyin, word, *freq);
        }
        if let Ok(mut slot) = self.custom.write() {
            *slot = custom;
        }
        if let Ok(mut slot) = self.codes.write() {
            *slot = codes;
        }
        if let Ok(mut slot) = self.phrases.write() {
            *slot = phrases;
        }
        if let Ok(mut slot) = self.custom_lexicon.write() {
            *slot = lexicon;
        }
        count
    }

    /// 全部自定义词，按 custom.tsv 里的顺序：(编码, 词)。词还是转义过的（`\n`），上屏前要
    /// [`crate::unescape_custom`]。
    pub fn custom_phrases(&self) -> Vec<(String, String)> {
        self.phrases.read().map(|p| p.clone()).unwrap_or_default()
    }

    /// 从文件读自定义词（整体替换）；文件不存在就清空。不是合法 UTF-8 的行跳过。
    pub fn load_custom<P: AsRef<Path>>(&self, path: P) -> Result<usize> {
        let path = path.as_ref();
        match crate::fsutil::read_lines_lossy(path) {
            Ok((lines, _bad)) => Ok(self.set_custom_tsv(&lines.join("\n"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(self.set_custom_tsv("")),
            Err(e) => Err(e).with_context(|| format!("读取自定义词失败: {}", path.display())),
        }
    }

    /// 是不是设置里添加的自定义词。
    pub fn is_custom(&self, word: &str) -> bool {
        self.custom
            .read()
            .map(|c| c.iter().any(|(_, w, _)| w == word))
            .unwrap_or(false)
    }

    /// 自定义编码对应的词（按文件里的顺序）。code 不分大小写、忽略隔音符。
    pub fn custom_code_words(&self, code: &str) -> Vec<String> {
        let code: String = code
            .chars()
            .filter(|c| *c != '\'')
            .flat_map(char::to_lowercase)
            .collect();
        self.codes
            .read()
            .map(|codes| {
                codes
                    .iter()
                    .filter(|(c, _)| *c == code)
                    .map(|(_, w)| w.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn custom_words(&self) -> Vec<(String, String, f64)> {
        self.custom.read().map(|w| w.clone()).unwrap_or_default()
    }

    /// 在用户词的词典、自定义词的词典里各查一次。
    fn in_overlays<T: Default>(&self, f: impl Fn(&MemoryLexicon) -> T) -> (T, T) {
        let user = self.user.read().map(|u| f(&u.lexicon)).unwrap_or_default();
        let custom = self
            .custom_lexicon
            .read()
            .map(|c| f(&c))
            .unwrap_or_default();
        (user, custom)
    }

    /// 用户词、自定义词里查到的（接在基础词库查到的后面）。用户词和自定义词都很少，复制没关系。
    fn overlay_lookup(&self, query: &[SyllableQuery], common: bool) -> Vec<LexEntry> {
        if common {
            let (mut user, custom) = self.in_overlays(|lex| lex.lookup_common(query));
            user.extend(custom);
            return user;
        }
        let (user, custom) = self.in_overlays(|lex| lex.lookup(query));
        if custom.is_empty() || !user.iter().any(LexEntry::is_rare) {
            let mut result = user;
            result.extend(custom);
            result
        } else {
            // 用户词和自定义词以前在同一份词典里，生僻字排在两样的最后；分成两份以后顺序照旧
            let (mut result, user_rare): (Vec<_>, Vec<_>) =
                user.into_iter().partition(|e| !e.is_rare());
            let (custom, custom_rare): (Vec<_>, Vec<_>) =
                custom.into_iter().partition(|e| !e.is_rare());
            result.extend(custom);
            result.extend(user_rare);
            result.extend(custom_rare);
            result
        }
    }
}

impl Lexicon for UserLexicon {
    fn lookup(&self, query: &[SyllableQuery]) -> Vec<LexEntry> {
        let mut result = self.base.lookup(query);
        result.extend(self.overlay_lookup(query, false));
        result
    }

    fn lookup_common(&self, query: &[SyllableQuery]) -> Vec<LexEntry> {
        let mut result = self.base.lookup_common(query);
        result.extend(self.overlay_lookup(query, true));
        result
    }

    fn visit(&self, query: &[SyllableQuery], common: bool, f: &mut dyn FnMut(&str, f64)) {
        self.base.visit(query, common, f);
        self.overlay_lookup(query, common)
            .iter()
            .for_each(|e| f(&e.word, e.freq));
    }

    fn has_longer(&self, query: &[SyllableQuery]) -> bool {
        self.base.has_longer(query)
            || self
                .user
                .read()
                .map_or(true, |u| u.lexicon.has_longer(query))
            || self
                .custom_lexicon
                .read()
                .map_or(true, |c| c.has_longer(query))
    }

    fn prepare_fuzzy(&self) {
        self.base.prepare_fuzzy();
    }

    fn lookup_fuzzy(&self, query: &[SyllableQuery], groups: u32) -> Vec<LexEntry> {
        let mut result = self.base.lookup_fuzzy(query, groups);
        let (user, custom) = self.in_overlays(|lex| lex.lookup_fuzzy(query, groups));
        result.extend(user);
        result.extend(custom);
        result
    }

    fn total_freq(&self) -> f64 {
        let (user, custom) = self.in_overlays(|lex| lex.total_freq());
        self.base.total_freq() + (user + custom)
    }

    fn len(&self) -> usize {
        let (user, custom) = self.in_overlays(|lex| lex.len());
        self.base.len() + user + custom
    }
}

/// 查询每个音节的声母（全拼音节取首字母，和词条的 initials 一样）。
fn query_initials(query: &[SyllableQuery]) -> Option<Vec<u8>> {
    query
        .iter()
        .map(|q| match q {
            SyllableQuery::Full { id, .. } => Syllable::from_id(*id).map(|s| s.initial_letter()),
            SyllableQuery::Jianpin { letter } => {
                Some(crate::syllable::jianpin_first_letter(*letter))
            }
        })
        .collect()
}

/// 简拼查询要不要逐条核对声母（zh/ch/sh 的简拼只按首字母查不够）。
fn needs_initial_check(query: &SyllableQuery) -> bool {
    matches!(query, SyllableQuery::Jianpin { letter }
        if crate::syllable::jianpin_required_initial(*letter).is_some())
}

fn initial_hash_step(hash: u64, letter: u8) -> u64 {
    (hash ^ u64::from(letter)).wrapping_mul(0x1000_0000_01b3)
}

/// 词条的音节键（索引里的写法）对不对得上一个全拼混简拼的查询。
fn keys_match(keys: &[u16], query: &[SyllableQuery]) -> bool {
    keys.len() == query.len()
        && query.iter().zip(keys).all(|(q, &raw)| match q {
            SyllableQuery::Full { id, erhua } => raw == raw_key(SyllableKey::new(*id, *erhua)),
            SyllableQuery::Jianpin { letter } => {
                match crate::syllable::jianpin_required_initial(*letter) {
                    Some(initial) => {
                        Syllable::from_id(raw & 0x7fff).is_some_and(|s| s.initial == initial)
                    }
                    None => initial_of(raw) == *letter,
                }
            }
        })
}

/// 解析词表里的拼音字段。
///
/// 允许：
/// - 单音节：xian
/// - 多音节：xi an / xi'an
/// - 儿化：hua r
///
/// 多音节不允许写成无分隔的连写，因为无法唯一切分（如 fangan）。
/// 把连着写的拼音切成音节（`nihao` -> `ni hao`）：音节数最少，一样多时前面的音节取长的
/// （`fangan` -> `fang an`，想要 fan gan 就自己加空格或隔音符）。切不开返回 None。
pub fn split_syllables(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    let n = text.len();
    // best[i]：text[i..] 最少切成几个音节，以及第一个音节的长度
    let mut best: Vec<Option<(usize, usize)>> = vec![None; n + 1];
    best[n] = Some((0, 0));
    for i in (0..n).rev() {
        for len in (1..=crate::syllable::MAX_SYLLABLE_LEN.min(n - i)).rev() {
            if !crate::syllable::is_valid(&text[i..i + len]) {
                continue;
            }
            if let Some((count, _)) = best[i + len] {
                if best[i].is_none_or(|(c, _)| count + 1 < c) {
                    best[i] = Some((count + 1, len));
                }
            }
        }
    }
    best[0]?;
    let mut parts = Vec::new();
    let mut i = 0;
    while i < n {
        let (_, len) = best[i]?;
        parts.push(&text[i..i + len]);
        i += len;
    }
    Some(parts.join(" "))
}

/// 自定义词的编码（设置界面加词、导入用）：拼音整理成空格分开的音节（见 [`normalize_pinyin`]）；
/// 不是拼音但全是字母的（mail）去掉隔音符、小写。都不是返回 None。
pub fn normalize_code(text: &str) -> Option<String> {
    if let Some(pinyin) = normalize_pinyin(text) {
        return Some(pinyin);
    }
    let code: String = text
        .trim()
        .to_lowercase()
        .chars()
        .filter(|&c| c != '\'')
        .collect();
    (!code.is_empty() && code.bytes().all(|b| b.is_ascii_lowercase())).then_some(code)
}

/// 把打的拼音整理成空格分开的音节：Ni Hǎo、nihao、ni'hao、ni3hao3 -> ni hao。
/// ü、u:、u 加分音符都写成 v；数字声调、单独的声调符号去掉；单独一个 r 跟在音节后面是儿化。
/// 连着写的照 [`split_syllables`] 切。有一段切不开（不是拼音的字母、符号）返回 None。
pub fn normalize_pinyin(text: &str) -> Option<String> {
    let mut plain = String::new();
    for c in text.trim().to_lowercase().chars() {
        if let Some(base) = toneless(c) {
            plain.push(base);
        } else if (c == '\u{308}' || c == ':') && plain.ends_with('u') {
            plain.pop();
            plain.push('v');
        } else if c.is_ascii_digit() || ('\u{300}'..='\u{36f}').contains(&c) {
            continue;
        } else {
            plain.push(c);
        }
    }
    let mut parts: Vec<String> = Vec::new();
    let tokens = plain
        .split(|c: char| c == '\'' || c == '\u{2019}' || c.is_whitespace())
        .filter(|t| !t.is_empty());
    for token in tokens {
        if token.bytes().all(|b| b.is_ascii_lowercase()) && crate::syllable::is_valid(token) {
            parts.push(token.to_string());
        } else if token == "r" && !parts.is_empty() {
            parts.push("r".to_string());
        } else {
            parts.push(split_syllables(token)?);
        }
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// 在词库原文（拼音<TAB>词<TAB>频次，一行一条）里猜词的拼音（设置界面加词时自动填编码）：
/// 整个词有就取频次最高的那条，没有就一个字一个字找。猜不出（有词库里没有的字、英文）返回 None。
/// 结果照 [`normalize_pinyin`] 整理过。
pub fn guess_pinyin(lexicon: &[u8], word: &str) -> Option<String> {
    if word.is_empty() {
        return None;
    }
    let chars: Vec<&str> = word
        .char_indices()
        .map(|(i, c)| &word[i..i + c.len_utf8()])
        .collect();
    // 词里出现过的不同的字：叠字（妈妈咪、试试看）共用一个槽位，按位置分槽的话第二个「妈」永远空着
    let mut uniq: Vec<&str> = Vec::new();
    for c in &chars {
        if !uniq.contains(c) {
            uniq.push(c);
        }
    }
    // best[0] 是整个词，best[1..] 是每个不同的字：(频次, 拼音)
    let mut best: Vec<Option<(f64, &[u8])>> = vec![None; uniq.len() + 1];
    for line in lexicon.split(|&b| b == b'\n') {
        if line.first() == Some(&b'#') {
            continue;
        }
        let mut cols = line.splitn(3, |&b| b == b'\t');
        let (Some(reading), Some(text), Some(freq)) = (cols.next(), cols.next(), cols.next())
        else {
            continue;
        };
        let slot = if text == word.as_bytes() {
            0
        } else if let Some(i) = uniq.iter().position(|c| c.as_bytes() == text) {
            i + 1
        } else {
            continue;
        };
        let freq = std::str::from_utf8(freq)
            .ok()
            .and_then(|f| f.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        if best[slot].is_none_or(|(old, _)| freq > old) {
            best[slot] = Some((freq, reading));
        }
    }
    let text = |reading: &[u8]| String::from_utf8_lossy(reading).into_owned();
    if let Some((_, reading)) = best[0] {
        return normalize_pinyin(&text(reading));
    }
    let readings: Option<Vec<String>> = chars
        .iter()
        .map(|c| {
            let slot = uniq.iter().position(|u| u == c)? + 1;
            best[slot].map(|(_, r)| text(r))
        })
        .collect();
    normalize_pinyin(&readings?.join(" "))
}

/// 带声调的字母 -> 不带的（ü 是 v）
fn toneless(c: char) -> Option<char> {
    Some(match c {
        'ā' | 'á' | 'ǎ' | 'à' => 'a',
        'ē' | 'é' | 'ě' | 'è' | 'ê' => 'e',
        'ī' | 'í' | 'ǐ' | 'ì' => 'i',
        'ō' | 'ó' | 'ǒ' | 'ò' => 'o',
        'ū' | 'ú' | 'ǔ' | 'ù' => 'u',
        'ü' | 'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' => 'v',
        'ń' | 'ň' | 'ǹ' => 'n',
        'ḿ' => 'm',
        _ => return None,
    })
}

/// 把带儿化标记的音节展开成「音节 + er」（wan(儿化) -> wan er）；没有儿化标记时返回 None。
fn erhua_spelled_out(keys: &[SyllableKey]) -> Option<Vec<SyllableKey>> {
    if !keys.iter().any(|k| k.erhua) {
        return None;
    }
    let er = Syllable::from_text("er")?.id;
    let mut out = Vec::with_capacity(keys.len() + 1);
    for key in keys {
        out.push(SyllableKey::new(key.id, false));
        if key.erhua {
            out.push(SyllableKey::new(er, false));
        }
    }
    Some(out)
}

pub fn parse_pinyin_keys(pinyin: &str) -> Result<Vec<SyllableKey>> {
    let pinyin = pinyin.trim();
    if pinyin.is_empty() {
        bail!("拼音不能为空");
    }

    // 单音节优先：xian 必须解析成一个音节，而不是 xi + an。
    if !pinyin.contains([' ', '\t', '\'']) {
        let key = erhua_token(pinyin)
            .ok_or_else(|| anyhow!("未知音节: {pinyin}（多音节请用空格分隔）"))?;
        return Ok(vec![key]);
    }

    let mut keys: Vec<SyllableKey> = Vec::new();
    for token in pinyin.split([' ', '\t', '\'']) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if token == "r" {
            match keys.last_mut() {
                Some(last) if !last.erhua => last.erhua = true,
                Some(_) => bail!("重复的儿化标记"),
                None => bail!("儿化标记 r 前面没有音节"),
            }
            continue;
        }
        keys.push(erhua_token(token).ok_or_else(|| anyhow!("未知音节: {token}"))?);
    }
    if keys.is_empty() {
        bail!("拼音不能为空");
    }
    Ok(keys)
}

/// 一个音节，或者带儿化的音节（wanr、dianr：候选的拼音是按输入拼的，选够次数收进用户词库时就是这样）。
fn erhua_token(token: &str) -> Option<SyllableKey> {
    if let Some(syl) = Syllable::from_text(token) {
        return Some(SyllableKey::new(syl.id, false));
    }
    let base = token.strip_suffix('r').filter(|b| !b.is_empty())?;
    Syllable::from_text(base).map(|syl| SyllableKey::new(syl.id, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex() -> MemoryLexicon {
        MemoryLexicon::from_tsv_str(
            "xian\t先\t1000\nxi an\t西安\t500\nzhong guo\t中国\t800\nhua r\t花儿\t100\n",
        )
        .unwrap()
    }

    /// is_ext_char 二分查 COMMON_EXT_CHARS（要求它全是四字节、按码位排好序）：和逐个找的结果一样。
    #[test]
    fn ext_char_lookup_matches_linear_search() {
        let chars: Vec<char> = COMMON_EXT_CHARS.chars().collect();
        assert!(chars.iter().all(|c| c.len_utf8() == 4));
        assert!(
            chars.windows(2).all(|w| w[0] < w[1]),
            "COMMON_EXT_CHARS 要按码位排好序"
        );
        let entry = |word: String| LexEntry {
            word,
            freq: 1.0,
            keys: Vec::new(),
            initials: Vec::new(),
        };
        let linear = |word: &str| {
            word.chars().count() == 1
                && u32::from(word.chars().next().unwrap()) >= 0x20000
                && !COMMON_EXT_CHARS.contains(word)
        };
        let samples = (0x4E00..0x4F00)
            .chain(0x1F600..0x1F700)
            .chain(0x1FFF0..0x32400)
            .filter_map(char::from_u32)
            .map(String::from)
            .chain(["𩽾𩾌", "a", "", "中文"].map(String::from));
        for word in samples {
            assert_eq!(entry(word.clone()).is_ext_char(), linear(&word), "{word}");
        }
    }

    #[test]
    fn full_lookup() {
        let lex = lex();
        let xian = Syllable::from_text("xian").unwrap();
        let found = lex.lookup(&[xian.query(false)]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].word, "先");
    }

    #[test]
    fn apostrophe_and_multi_syllable() {
        let lex = lex();
        let xi = Syllable::from_text("xi").unwrap();
        let an = Syllable::from_text("an").unwrap();
        let found = lex.lookup(&[xi.query(false), an.query(false)]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].word, "西安");
    }

    #[test]
    fn mixed_lookup_and_prune() {
        let mut lex = MemoryLexicon::new();
        lex.insert("zhong wen", "中文", 100.0).unwrap();
        lex.insert("zheng wen", "正文", 50.0).unwrap();
        lex.insert("zhong guo ren", "中国人", 80.0).unwrap();
        let wen = Syllable::from_text("wen").unwrap().query(false);
        let guo = Syllable::from_text("guo").unwrap().query(false);
        let z = SyllableQuery::Jianpin { letter: b'z' };
        let words: Vec<String> = lex.lookup(&[z, wen]).into_iter().map(|e| e.word).collect();
        assert_eq!(words.len(), 2, "{words:?}");
        assert!(words.contains(&"中文".to_string()));
        // 剪枝按声母：z guo 后面还有中国人，z wen 后面没有更长的词
        assert!(lex.has_longer(&[z, guo]));
        assert!(!lex.has_longer(&[z, wen]));
        assert!(lex.has_longer(&[z]));
        assert!(!lex.has_longer(&[SyllableQuery::Jianpin { letter: b'q' }]));
    }

    #[test]
    fn jianpin_lookup() {
        let lex = lex();
        let found = lex.lookup(&[
            SyllableQuery::Jianpin { letter: b'z' },
            SyllableQuery::Jianpin { letter: b'g' },
        ]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].word, "中国");
    }

    #[test]
    fn digraph_jianpin_lookup() {
        let mut lex = MemoryLexicon::new();
        lex.insert("zhi chi", "支持", 100.0).unwrap();
        lex.insert("zi chi", "自持", 10.0).unwrap();
        lex.insert("zhi ci", "至此", 50.0).unwrap();
        let words = |q: &[SyllableQuery]| {
            let mut w: Vec<String> = lex.lookup(q).into_iter().map(|e| e.word).collect();
            w.sort();
            w
        };
        let zh = SyllableQuery::Jianpin { letter: b'Z' };
        let ch = SyllableQuery::Jianpin { letter: b'C' };
        let z = SyllableQuery::Jianpin { letter: b'z' };
        let c = SyllableQuery::Jianpin { letter: b'c' };
        let chi = Syllable::from_text("chi").unwrap().query(false);
        assert_eq!(words(&[zh, ch]), vec!["支持"]);
        assert_eq!(words(&[zh, c]), vec!["支持", "至此"]);
        assert_eq!(words(&[z, c]), vec!["支持", "自持", "至此"]);
        assert_eq!(words(&[zh, chi]), vec!["支持"]);
        assert_eq!(words(&[z, chi]), vec!["支持", "自持"]);
        assert!(lex.has_longer(&[zh]));
    }

    #[test]
    fn erhua_lookup() {
        let lex = lex();
        let hua = Syllable::from_text("hua").unwrap();
        let found = lex.lookup(&[hua.query(true)]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].word, "花儿");
        assert!(lex.lookup(&[hua.query(false)]).is_empty());
    }

    #[test]
    fn erhua_pinyin_parses() {
        let wan = Syllable::from_text("wan").unwrap().id;
        let dian = Syllable::from_text("dian").unwrap().id;
        assert_eq!(
            parse_pinyin_keys("wanr").unwrap(),
            [SyllableKey::new(wan, true)]
        );
        assert_eq!(
            parse_pinyin_keys("wan r").unwrap(),
            [SyllableKey::new(wan, true)]
        );
        assert_eq!(
            parse_pinyin_keys("yi dianr").unwrap()[1],
            SyllableKey::new(dian, true)
        );
        assert_eq!(parse_pinyin_keys("er").unwrap().len(), 1, "er 本身是个音节");
        assert!(parse_pinyin_keys("r").is_err());
        // 收进用户词库以后，wanr 直接查得到
        let mut lex = MemoryLexicon::new();
        lex.insert("wanr", "玩儿", 1000.0).unwrap();
        let q = Syllable::from_text("wan").unwrap().query(true);
        assert_eq!(lex.lookup(&[q])[0].word, "玩儿");
    }

    #[test]
    fn erhua_matches_words_spelled_with_er() {
        let lex = MemoryLexicon::from_tsv_str(concat!(
            "wan er\t玩儿\t75000\n",
            "wan er\t万二\t10\n",
            "yi dian er\t一点儿\t30000\n",
            "wan er ming\t玩儿命\t2000\n",
        ))
        .unwrap();
        let q = |s: &str, erhua: bool| Syllable::from_text(s).unwrap().query(erhua);
        let words = |query: &[SyllableQuery]| -> Vec<String> {
            lex.lookup(query).into_iter().map(|e| e.word).collect()
        };
        // wanr：只要带「儿」的，同音的「万二」不算儿化
        assert_eq!(words(&[q("wan", true)]), ["玩儿"]);
        assert_eq!(words(&[q("yi", false), q("dian", true)]), ["一点儿"]);
        assert!(lex.has_longer(&[q("wan", true)]), "玩儿命 要能接着往后查");
        assert_eq!(words(&[q("wan", true), q("ming", false)]), ["玩儿命"]);
        assert!(words(&[q("wan", false)]).is_empty());
    }

    /// visit（不复制词条）逐条给出的和 lookup / lookup_common 返回的一样、先后一样：
    /// 内存词典的全拼、儿化、简拼、混简拼、zh 简拼，用户词库叠用户词和自定义词（生僻字排最后），模糊音视图。
    #[test]
    fn visit_matches_lookup() {
        let base = MemoryLexicon::from_tsv_str(concat!(
            "wan er\t玩儿\t75000\n",
            "wan er\t万二\t10\n",
            "zhong guo\t中国\t800\n",
            "zhong guo\t种果\t0.5\n",
            "zhong\t中\t9000\n",
            "zhong\t𠁥\t0.2\n",
            "zong\t总\t7000\n",
            "zhi chi\t支持\t600\n",
            "ke yi\t可以\t900\n",
        ))
        .unwrap();
        let user = UserLexicon::new(Arc::new(base));
        user.add("zhong", "钟", 300.0).unwrap();
        user.add("zhong", "𠁩", 0.3).unwrap();
        user.add("zhong guo", "中果", 20.0).unwrap();
        user.set_custom_tsv("zhong\t忠\t40\nzhong\t𠁪\t0.4\n");
        let q = |s: &str, erhua: bool| Syllable::from_text(s).unwrap().query(erhua);
        let j = |letter: u8| SyllableQuery::Jianpin { letter };
        let queries: Vec<Vec<SyllableQuery>> = vec![
            vec![q("zhong", false)],
            vec![q("zhong", false), q("guo", false)],
            vec![q("wan", true)],
            vec![j(b'z'), j(b'g')],
            vec![j(b'z')],
            vec![q("ke", false), j(b'y')],
            vec![j(b'k'), q("yi", false)],
            vec![q("zhi", false), j(b'c')],
            vec![q("zong", false)],
            vec![],
        ];
        let fuzzy = FuzzyLexicon::new(&user, crate::syllable::fuzzy::ALL, 1.0);
        let views: [&dyn Lexicon; 2] = [&user, &fuzzy];
        for lex in views.into_iter().chain([user.base.as_ref()]) {
            for query in &queries {
                for common in [false, true] {
                    let mut seen = Vec::new();
                    lex.visit(query, common, &mut |w, f| seen.push((w.to_string(), f)));
                    let want: Vec<_> = if common {
                        lex.lookup_common(query)
                    } else {
                        lex.lookup(query)
                    }
                    .into_iter()
                    .map(|e| (e.word, e.freq))
                    .collect();
                    assert_eq!(seen, want, "{query:?} common={common}");
                }
            }
        }
        // 用户词里有生僻字、又有自定义词时，生僻字排在两样的最后
        let mut seen = Vec::new();
        user.visit(&queries[0], false, &mut |w, _| seen.push(w.to_string()));
        assert_eq!(seen, ["中", "𠁥", "钟", "忠", "𠁩", "𠁪"]);
    }

    #[test]
    fn rejects_ambiguous_concatenation() {
        assert!(parse_pinyin_keys("fangan").is_err());
    }

    #[test]
    fn total_freq_and_cost() {
        let lex = lex();
        assert_eq!(lex.total_freq(), 2400.0);
        let entry = &lex.entry(0);
        assert!(lex.cost(entry) > 0.0);
    }

    #[test]
    fn lenient_loading_skips_unknown_syllables() {
        // 「qqq」不是音节，整行跳过而不是让整个词库失败
        let lex = MemoryLexicon::from_tsv_str("ni hao\t你好\t100\nqqq\t呲\t5\n").unwrap();
        assert_eq!(lex.len(), 1);
        assert_eq!(lex.skipped(), 1);
    }

    #[test]
    fn structural_errors_still_fail() {
        assert!(MemoryLexicon::from_tsv_str("只有两列\t词\n").is_err());
        assert!(MemoryLexicon::from_tsv_str("ni hao\t你好\tabc\n").is_err());
    }

    fn temp_lexicon(tag: &str, body: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("archtype_lex_{}_{}.tsv", tag, std::process::id()));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(cache_path(&path));
        std::fs::write(&path, body).expect("写临时词库失败");
        path
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(cache_path(path));
    }

    #[test]
    fn split_syllables_prefers_fewest_and_longest_first() {
        assert_eq!(split_syllables("nihao").as_deref(), Some("ni hao"));
        assert_eq!(split_syllables("fangan").as_deref(), Some("fang an"));
        assert_eq!(split_syllables("xian").as_deref(), Some("xian"));
        assert_eq!(
            split_syllables("zhongguoren").as_deref(),
            Some("zhong guo ren")
        );
        assert_eq!(split_syllables("github"), None);
        assert_eq!(split_syllables("Ni"), None);
    }

    #[test]
    fn normalize_custom_codes() {
        for (input, want) in [
            ("nihao", "ni hao"),
            ("Ni Hǎo", "ni hao"),
            ("ni3hao3", "ni hao"),
            ("xi'an", "xi an"),
            ("xi\u{2019}an", "xi an"),
            ("xian", "xian"),
            ("Lǜ shī", "lv shi"),
            ("nu:", "nv"),
            ("nu\u{308}3", "nv"),
            ("hua r", "hua r"),
            ("zhongguo", "zhong guo"),
            ("  biang biang mian ", "biang biang mian"),
        ] {
            assert_eq!(normalize_pinyin(input).as_deref(), Some(want), "{input}");
        }
        assert_eq!(normalize_pinyin("mail"), None);
        assert_eq!(normalize_pinyin("r"), None);
        assert_eq!(normalize_pinyin(""), None);
        assert_eq!(normalize_code("mail").as_deref(), Some("mail"));
        assert_eq!(normalize_code("Addr").as_deref(), Some("addr"));
        assert_eq!(normalize_code("o'k").as_deref(), Some("ok"));
        assert_eq!(normalize_code("a b?"), None);
        assert_eq!(normalize_code("  "), None);
    }

    #[test]
    fn guess_pinyin_from_lexicon_text() {
        let lexicon = "# 注释\tshi\t是\t1\nni hao\t你好\t500\nnihao\t你好\t10\nzhong\t重\t100\n\
                       chong\t重\t300\nni\t你\t1\nhao\t好\t1\nshi\t是\t9\r\nlv\t绿\t5";
        let guess = |word: &str| guess_pinyin(lexicon.as_bytes(), word);
        assert_eq!(guess("你好").as_deref(), Some("ni hao"));
        assert_eq!(guess("重").as_deref(), Some("chong"));
        assert_eq!(guess("你是好").as_deref(), Some("ni shi hao"));
        assert_eq!(guess("绿").as_deref(), Some("lv"));
        assert_eq!(guess("你x"), None);
        // 叠字：整词不在词库里，逐字找时两个相同的字都有读音
        assert_eq!(guess("你你好").as_deref(), Some("ni ni hao"));
        assert_eq!(guess("好好").as_deref(), Some("hao hao"));
        assert_eq!(guess("你你x"), None);
        assert_eq!(guess(""), None);
    }

    #[test]
    fn cache_roundtrip_matches_tsv() {
        let path = temp_lexicon("rt", "ni hao\t你好\t100\nzhong guo\t中国\t50\n");
        let first = MemoryLexicon::from_tsv_path_cached(&path).unwrap();
        assert!(cache_path(&path).exists(), "第一次加载应当写出快照");
        let second = MemoryLexicon::from_tsv_path_cached(&path).unwrap();
        assert_eq!(first.len(), second.len());
        assert_eq!(first.all_entries(), second.all_entries());
        assert_eq!(first.total_freq, second.total_freq);
        cleanup(&path);
    }

    #[test]
    fn stale_cache_is_ignored() {
        let path = temp_lexicon("stale", "ni hao\t你好\t100\n");
        let first = MemoryLexicon::from_tsv_path_cached(&path).unwrap();
        assert_eq!(first.len(), 1);
        // 源文件变了（内容更长），旧快照必须失效
        std::fs::write(
            &path,
            "ni hao\t你好\t100\nzhong guo\t中国\t50\nshi jie\t世界\t30\n",
        )
        .unwrap();
        let second = MemoryLexicon::from_tsv_path_cached(&path).unwrap();
        assert_eq!(second.len(), 3, "源文件更新后应当重新解析");
        cleanup(&path);
    }

    #[test]
    fn cache_from_other_syllable_table_is_ignored() {
        // 音节表改了（版本号还是旧的那个）：快照里的音节 ID 对不上，必须重新解析
        let path = temp_lexicon("table", "ni hao\t你好\t100\n");
        let lex = MemoryLexicon::from_tsv_path(&path).unwrap();
        let (len, mtime) = source_stamp(&path).unwrap();
        let cache = cache_path(&path);
        lex.write_cache(&cache, len, mtime).unwrap();
        let mut data = std::fs::read(&cache).unwrap();
        assert!(MemoryLexicon::read_cache(&data, len, mtime).is_some());
        data[4..8].copy_from_slice(&CACHE_VERSION.to_le_bytes());
        assert!(MemoryLexicon::read_cache(&data, len, mtime).is_none());
        cleanup(&path);
    }

    #[test]
    fn corrupt_cache_falls_back_to_tsv() {
        let path = temp_lexicon("corrupt", "ni hao\t你好\t100\n");
        let _ = MemoryLexicon::from_tsv_path_cached(&path).unwrap();
        std::fs::write(cache_path(&path), b"not a cache").unwrap();
        let lex = MemoryLexicon::from_tsv_path_cached(&path).unwrap();
        assert_eq!(lex.len(), 1, "快照损坏时应回退到 TSV");
        cleanup(&path);
    }

    /// 一个小词库：全拼同音、简拼同声母、儿化、生僻字、模糊音都有。
    const MIXED: &str = concat!(
        "zhong guo\t中国\t800\nzhong guo\t种果\t0.5\nzhong\t中\t9000\nzhong\t𠁥\t0.2\n",
        "zhong\t钟\t3000\nzong\t总\t7000\nzhi chi\t支持\t600\nzi chi\t自持\t10\n",
        "ke yi\t可以\t900\nke\t可\t5000\nwan er\t玩儿\t75000\nwan er ming\t玩儿命\t30\n",
        "xi an\t西安\t500\nxian\t先\t1000\nsan\t三\t4000\nshang\t上\t8000\nhua r\t花儿\t100\n",
    );

    /// 查一个词典能查到的所有东西（全拼、简拼、混简拼、儿化、模糊音、has_longer），拿来比两份词典。
    fn probe_all(lex: &dyn Lexicon) -> Vec<String> {
        let q = |s: &str, erhua: bool| Syllable::from_text(s).unwrap().query(erhua);
        let j = |letter: u8| SyllableQuery::Jianpin { letter };
        let queries: Vec<Vec<SyllableQuery>> = vec![
            vec![q("zhong", false)],
            vec![q("zhong", false), q("guo", false)],
            vec![q("zhong", false), j(b'g')],
            vec![j(b'z'), j(b'g')],
            vec![j(b'z')],
            vec![j(b'Z')],
            vec![j(b'k'), j(b'y')],
            vec![q("ke", false), j(b'y')],
            vec![q("zhi", false), j(b'c')],
            vec![q("wan", true)],
            vec![q("wan", true), q("ming", false)],
            vec![q("hua", true)],
            vec![q("xi", false)],
            vec![q("xian", false)],
            vec![q("zong", false)],
            vec![q("san", false)],
            vec![q("zhi", false)],
            vec![],
        ];
        let mut out = Vec::new();
        for query in &queries {
            for e in lex.lookup(query) {
                out.push(format!("L {query:?} {e:?}"));
            }
            for e in lex.lookup_common(query) {
                out.push(format!("C {query:?} {e:?}"));
            }
            for e in lex.lookup_fuzzy(query, crate::syllable::fuzzy::ALL) {
                out.push(format!("F {query:?} {e:?}"));
            }
            out.push(format!("H {query:?} {}", lex.has_longer(query)));
        }
        out.push(format!("T {} {}", lex.total_freq(), lex.len()));
        out
    }

    /// 从快照载入（映射进来的）和从 TSV 建的查什么都一样。
    #[test]
    fn cache_lookups_match_tsv() {
        let path = temp_lexicon("probe", MIXED);
        let fresh = MemoryLexicon::from_tsv_path(&path).unwrap();
        let first = MemoryLexicon::from_tsv_path_cached(&path).unwrap();
        assert!(first.blob.is_none(), "第一次是从 TSV 建的");
        let cached = MemoryLexicon::from_tsv_path_cached(&path).unwrap();
        assert!(cached.blob.is_some(), "第二次应当读快照");
        assert_eq!(probe_all(&fresh), probe_all(&cached));
        assert_eq!(probe_all(&fresh), probe_all(&first));
        cleanup(&path);
    }

    /// 建好以后再 insert 的，和从头一个一个 insert 的查起来一样（生僻字照样排在每组最后）。
    #[test]
    fn insert_after_build_matches_insert_from_scratch() {
        let extra = [
            ("zhong", "忠", 40.0),
            ("zhong", "𠁩", 0.3),
            ("zhong guo", "中果", 20.0),
            ("zhong gu", "钟鼓", 5.0),
            ("zi", "字", 900.0),
            ("ke yi", "科艺", 0.5),
        ];
        let mut built = MemoryLexicon::from_tsv_str(MIXED).unwrap();
        let mut scratch = MemoryLexicon::new();
        for line in MIXED.lines() {
            let mut cols = line.split('\t');
            let (pinyin, word, freq) = (
                cols.next().unwrap(),
                cols.next().unwrap(),
                cols.next().unwrap(),
            );
            scratch.insert(pinyin, word, freq.parse().unwrap()).unwrap();
        }
        assert_eq!(probe_all(&built), probe_all(&scratch));
        for (pinyin, word, freq) in extra {
            built.insert(pinyin, word, freq).unwrap();
            scratch.insert(pinyin, word, freq).unwrap();
            assert_eq!(probe_all(&built), probe_all(&scratch), "{word}");
        }
    }

    /// 快照坏了的各种样子：截短、改了一个字节（校验和对不上）、结构不对但校验和是对的，都不能用，
    /// 也不能崩。
    #[test]
    fn damaged_cache_is_rejected() {
        let path = temp_lexicon("damaged", MIXED);
        let lex = MemoryLexicon::from_tsv_path(&path).unwrap();
        let (len, mtime) = source_stamp(&path).unwrap();
        let cache = cache_path(&path);
        lex.write_cache(&cache, len, mtime).unwrap();
        let data = std::fs::read(&cache).unwrap();
        assert!(MemoryLexicon::read_cache(&data, len, mtime).is_some());
        for cut in [0, 10, CACHE_HEADER, data.len() / 2, data.len() - 1] {
            assert!(
                MemoryLexicon::read_cache(&data[..cut], len, mtime).is_none(),
                "截到 {cut}"
            );
        }
        for at in [CACHE_HEADER + 3, data.len() / 3, data.len() - 9] {
            let mut bad = data.clone();
            bad[at] ^= 0x40;
            assert!(
                MemoryLexicon::read_cache(&bad, len, mtime).is_none(),
                "改了第 {at} 字节"
            );
        }
        // 改了内容、重算校验和：结构检查也要拦下来（每一节都试，改成很大的数）
        let section = |data: &[u8], i: usize| {
            let at = CACHE_HEADER + i * 16;
            let off = u64::from_le_bytes(data[at..at + 8].try_into().unwrap()) as usize;
            let bytes = u64::from_le_bytes(data[at + 8..at + 16].try_into().unwrap()) as usize;
            (off, bytes)
        };
        let reseal = |data: &mut Vec<u8>| {
            let sum = crate::lexstore::checksum(&data[CACHE_HEADER..]);
            data[40..48].copy_from_slice(&sum.to_le_bytes());
        };
        let mut sealed = data.clone();
        reseal(&mut sealed);
        assert!(MemoryLexicon::read_cache(&sealed, len, mtime).is_some());
        for i in 0..CACHE_SECTIONS {
            let (off, bytes) = section(&data, i);
            // 频次、简拼的声母、两个前缀指纹表：什么数都不会出错
            if bytes < 4 || [2, 10, 14, 15].contains(&i) {
                continue;
            }
            let mut bad = data.clone();
            bad[off..off + 4].copy_from_slice(&0xfff0_fff0u32.to_le_bytes());
            reseal(&mut bad);
            assert!(
                MemoryLexicon::read_cache(&bad, len, mtime).is_none(),
                "第 {i} 节"
            );
        }
        // 节的位置越界
        let mut bad = data.clone();
        bad[CACHE_HEADER + 8..CACHE_HEADER + 16].copy_from_slice(&u64::MAX.to_le_bytes());
        reseal(&mut bad);
        assert!(MemoryLexicon::read_cache(&bad, len, mtime).is_none());
        // 词不是合法 UTF-8
        let (off, _) = section(&data, 0);
        let mut bad = data.clone();
        bad[off] = 0xff;
        reseal(&mut bad);
        assert!(MemoryLexicon::read_cache(&bad, len, mtime).is_none());
        // 写坏的快照放在旁边，载入时退回 TSV
        std::fs::write(&cache, &data[..data.len() / 2]).unwrap();
        let reloaded = MemoryLexicon::from_tsv_path_cached(&path).unwrap();
        assert_eq!(probe_all(&reloaded), probe_all(&lex));
        cleanup(&path);
    }

    #[test]
    fn placeholder_weight_words_get_estimated_from_their_parts() {
        let mut lex = MemoryLexicon::from_tsv_str(
            "bu\t不\t3000000\njian yi\t建议\t500000\nbu jian\t不见\t250000\nyi\t一\t35000000\n\
             bu jian yi\t不建议\t100\nbu jian ying\t不坚硬\t100\nhao\t好\t9000\n\
             zhe ge\t这个\t100\n",
        )
        .unwrap();
        let total = lex.total_freq;
        lex.estimate_placeholder_freqs(2);
        let entries = lex.all_entries();
        let freq = |w: &str| entries.iter().find(|e| e.word == w).unwrap().freq;
        // 不建议 = 不 × 建议 × 加成，高过「不见 + 一」这句
        let cost = |f: f64| -(f / total).ln();
        assert!(cost(freq("不建议")) < cost(freq("不见")) + cost(freq("一")));
        // 不会高过最冷门的那部分
        assert!(freq("不建议") <= 500000.0);
        // 切不开的词保持原样；总频次不变
        assert_eq!(freq("不坚硬"), 100.0);
        assert_eq!(freq("这个"), 100.0);
        assert_eq!(lex.total_freq, total);
    }

    fn user_lexicon() -> UserLexicon {
        UserLexicon::new(Arc::new(MemoryLexicon::new()))
    }

    fn full(text: &str) -> Vec<SyllableQuery> {
        text.split(' ')
            .map(|t| Syllable::from_text(t).unwrap().query(false))
            .collect()
    }

    fn freq_of(user: &UserLexicon, pinyin: &str, word: &str) -> Option<f64> {
        user.lookup(&full(pinyin))
            .into_iter()
            .find(|e| e.word == word)
            .map(|e| e.freq)
    }

    #[test]
    fn loading_user_words_rebuilds_once() {
        // 以前一行重建一次：3000 个词 7 秒。数重建的次数，不看时间
        let syllables = ["ni", "hao", "zhong", "guo", "ren", "min", "da", "xue"];
        let n = 500;
        let mut body = String::from("# 用户词库\n");
        for i in 0..n {
            let (a, b) = (syllables[i % 8], syllables[(i / 8) % 8]);
            body.push_str(&format!("{a} {b}\t词{i}\t{}\n", 1000 + i));
        }
        let path = temp_lexicon("user_bulk", &body);
        let user = user_lexicon();
        assert_eq!(user.load_tsv(&path).unwrap(), n);
        cleanup(&path);
        assert_eq!(user.rebuild_count(), 1, "载入 {n} 个词只建一次词典");
        assert_eq!(user.words().len(), n);
        assert_eq!(user.len(), n);
        assert_eq!(freq_of(&user, "ni ni", "词0"), Some(1000.0));
        assert_eq!(freq_of(&user, "hao xue", "词57"), Some(1057.0));
        assert_eq!(freq_of(&user, "guo da", "词499"), Some(1499.0));

        // 学过的词再上屏：只是频次变大，就地改
        user.add("ni ni", "词0", 5000.0).unwrap();
        assert_eq!(freq_of(&user, "ni ni", "词0"), Some(5000.0));
        // 拼音换了个写法，读音没变
        user.add("ni'ni", "词0", 6000.0).unwrap();
        assert_eq!(freq_of(&user, "ni ni", "词0"), Some(6000.0));
        // 新词接在最后
        user.add("da xue", "大学", 2000.0).unwrap();
        assert_eq!(freq_of(&user, "da xue", "大学"), Some(2000.0));
        assert_eq!(user.rebuild_count(), 1, "改频次、加新词都不重建");
        assert_eq!(user.words().len(), n + 1);

        // 删词、改拼音才从头建
        assert!(user.remove("词57"));
        assert_eq!(freq_of(&user, "hao xue", "词57"), None);
        assert_eq!(user.rebuild_count(), 2);
        user.add("xue da", "大学", 2000.0).unwrap();
        assert_eq!(freq_of(&user, "da xue", "大学"), None);
        assert_eq!(freq_of(&user, "xue da", "大学"), Some(2000.0));
        assert_eq!(user.rebuild_count(), 3);
        assert!(!user.remove("没有这个词"));
        assert_eq!(user.rebuild_count(), 3);

        // 空文件、只有注释的文件不用建
        let empty = temp_lexicon("user_empty", "# 用户词库\n");
        assert_eq!(user.load_tsv(&empty).unwrap(), 0);
        cleanup(&empty);
        assert_eq!(user.rebuild_count(), 3);
    }

    /// 以前的做法：用户词、自定义词写成一份 TSV，从头解析成一份词典。
    fn fresh_build(user: &UserLexicon) -> MemoryLexicon {
        let mut tsv = String::new();
        for (pinyin, word, freq) in user.words().into_iter().chain(user.custom_words()) {
            tsv.push_str(&format!("{pinyin}\t{word}\t{freq}\n"));
        }
        MemoryLexicon::from_tsv_str(&tsv).unwrap()
    }

    #[test]
    fn user_lexicon_matches_fresh_build() {
        // 就地改频次、接在最后加词、用户词和自定义词分成两份词典，查出来的东西（连顺序）
        // 都要和以前每次从头建的一样
        let syllables = ["ni", "hao", "shi", "si", "zhi", "zi", "an", "ang"];
        let mut queries: Vec<Vec<SyllableQuery>> = Vec::new();
        let letters = *b"nhsza";
        for a in syllables {
            queries.push(full(a));
            for b in syllables {
                queries.push(full(&format!("{a} {b}")));
            }
            for letter in letters {
                queries.push(vec![full(a)[0], SyllableQuery::Jianpin { letter }]);
                queries.push(vec![SyllableQuery::Jianpin { letter }, full(a)[0]]);
            }
        }
        for a in letters {
            queries.push(vec![SyllableQuery::Jianpin { letter: a }]);
            for b in letters {
                queries.push(vec![
                    SyllableQuery::Jianpin { letter: a },
                    SyllableQuery::Jianpin { letter: b },
                ]);
            }
        }
        let check = |user: &UserLexicon, step: &str| {
            let fresh = fresh_build(user);
            assert_eq!(user.len(), fresh.len(), "{step}");
            assert_eq!(user.total_freq(), fresh.total_freq(), "{step}");
            for q in &queries {
                assert_eq!(user.lookup(q), fresh.lookup(q), "{step} {q:?}");
                assert_eq!(
                    user.lookup_common(q),
                    fresh.lookup_common(q),
                    "{step} {q:?}"
                );
                assert_eq!(
                    user.lookup_fuzzy(q, crate::syllable::fuzzy::ALL),
                    fresh.lookup_fuzzy(q, crate::syllable::fuzzy::ALL),
                    "{step} {q:?}"
                );
                assert_eq!(user.has_longer(q), fresh.has_longer(q), "{step} {q:?}");
            }
        };

        // 单字（频次不到 1 就是生僻字，排在最后）和词都有；频次里有不合法的
        let words = [
            "妮", "是", "子", "你好", "只是", "自私", "暗示", "昂", "死", "指",
        ];
        let freqs = [0.25, 0.5, 1.0, 3.0, 1000.0, 2000.0, 50_000.0];
        let bad_freqs = [f64::NAN, f64::INFINITY, -1.0];
        let customs = [
            "",
            "ni\t妮\t0.5\nni\t你\nni hao\t妮好\nshi\t是\t2\n",
            "si\t死\t0.5\nsi\t四\nzhi shi\t只是\nmail\t邮箱\n",
        ];
        let user = user_lexicon();
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = |n: usize| {
            // xorshift：每次跑都是同一串
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        for step in 0..600 {
            let word = words[next(words.len())];
            let what = match next(10) {
                0 => {
                    user.remove(word);
                    format!("第 {step} 步：删 {word}")
                }
                1 => {
                    let custom = customs[next(customs.len())];
                    user.set_custom_tsv(custom);
                    format!("第 {step} 步：换自定义词 {custom:?}")
                }
                _ => {
                    // 同一个词大多用同一个拼音（只是频次变了），偶尔换一个
                    let at = if next(6) == 0 {
                        next(64)
                    } else {
                        word.len() * 7 + word.as_bytes()[1] as usize
                    };
                    let pinyin = if at % 3 == 0 {
                        syllables[at % 8].to_string()
                    } else {
                        // 隔音符和空格是一个读音的两种写法
                        let sep = if next(4) == 0 { "'" } else { " " };
                        format!("{}{sep}{}", syllables[at % 8], syllables[(at / 8) % 8])
                    };
                    let freq = if next(12) == 0 {
                        bad_freqs[next(bad_freqs.len())]
                    } else {
                        freqs[next(freqs.len())]
                    };
                    user.add(&pinyin, word, freq).unwrap();
                    format!("第 {step} 步：加 {pinyin} {word} {freq}")
                }
            };
            check(&user, &what);
        }
        user.clear();
        check(&user, "清空");
        assert!(user.words().is_empty());
    }

    #[test]
    fn user_word_with_tab_does_not_freeze_the_rest() {
        // 带制表符的词写不进 TSV。以前它让整份重建失败，之后加的词全都查不到
        let user = user_lexicon();
        user.add("ni hao", "你\t好", 1000.0).unwrap();
        user.add("ni hao", "你好", 1000.0).unwrap();
        let found = user.lookup(&full("ni hao"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].word, "你好");
        assert_eq!(user.words().len(), 2);
    }

    /// 三行，中间那行夹了一个不是 UTF-8 的字节。
    fn with_bad_line(first: &str, bad: &[u8], last: &str) -> Vec<u8> {
        let mut body = first.as_bytes().to_vec();
        body.extend_from_slice(bad);
        body.extend_from_slice(last.as_bytes());
        body
    }

    #[test]
    fn one_bad_byte_does_not_lose_the_user_dict() {
        // 以前 read_to_string 整份失败，外壳照常存盘，文件被这次会话的内容盖掉
        let path = temp_lexicon("user_badbyte", "");
        let body = with_bad_line(
            "# 用户词库\nni hao\t你好\t3000\n",
            b"zhong \xff\t\xe4\xb8\t2000\n",
            "shi jie\t世界\t1000\n",
        );
        std::fs::write(&path, body).unwrap();
        let user = user_lexicon();
        assert_eq!(user.load_tsv(&path).unwrap(), 2, "坏行跳过，别的行照常");
        cleanup(&path);
        assert_eq!(freq_of(&user, "ni hao", "你好"), Some(3000.0));
        assert_eq!(freq_of(&user, "shi jie", "世界"), Some(1000.0));
        assert_eq!(user.words().len(), 2);
        // 文件不存在还是报错（外壳靠它知道是第一次用）
        assert!(user.load_tsv(&path).is_err());
    }

    #[test]
    fn one_bad_byte_does_not_lose_custom_words() {
        let path = temp_lexicon("custom_badbyte", "");
        let body = with_bad_line(
            "ni hao\t妮好\n",
            b"wo de\t\xff\xfe\n",
            "mail\tme@example.com\n",
        );
        std::fs::write(&path, body).unwrap();
        let user = user_lexicon();
        assert_eq!(user.load_custom(&path).unwrap(), 2);
        cleanup(&path);
        assert!(user.is_custom("妮好"));
        assert_eq!(user.custom_code_words("mail"), ["me@example.com"]);
        assert_eq!(user.custom_phrases().len(), 2);
        // 文件不存在是清空，不是出错
        assert_eq!(user.load_custom(&path).unwrap(), 0);
        assert!(!user.is_custom("妮好"));
    }
}
