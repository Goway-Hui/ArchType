//! 词库的紧凑存储（#233）：一列一列的定长数组，可以直接指向映射进来的快照文件。
//!
//! 以前一个词条是一个 `LexEntry`（字、音节、声母三份堆分配），全拼、简拼、模糊音三张
//! `HashMap<Vec<_>, Vec<usize>>` 索引又各存一份键和下标表，93 万词的雾凇词库要 450 MB 左右。
//! 现在全部是 u32 下标数组：词的字放进一整块字节里、音节按「同一个读音一组」只存一份，
//! 索引是开放寻址的 u32 槽 + 每组一段下标（CSR）。快照文件就是这些数组原样写下来，
//! 载入时映射进来直接用，不再逐条解析、不再在堆里复制一份。

use std::ops::Deref;

/// 一列数据：自己的 `Vec`，或者指向映射进来的快照（[`Blob`]）里的一段。
///
/// `Mapped` 指向的内存由持有它的词典里那个 `Arc<Blob>` 保活，所以只能放在同时持有 `Blob`
/// 的结构里（`MemoryLexicon`），而且要声明在 `Blob` 前面（先释放）。
pub(crate) enum Arr<T: Copy> {
    Owned(Vec<T>),
    Mapped { ptr: *const T, len: usize },
}

// 映射的内存只读、不会被别人改（快照是写好临时文件再改名换上的），跨线程读没有问题
unsafe impl<T: Copy + Send> Send for Arr<T> {}
unsafe impl<T: Copy + Sync> Sync for Arr<T> {}

impl<T: Copy> Default for Arr<T> {
    fn default() -> Self {
        Arr::Owned(Vec::new())
    }
}

impl<T: Copy> From<Vec<T>> for Arr<T> {
    fn from(v: Vec<T>) -> Self {
        Arr::Owned(v)
    }
}

impl<T: Copy> Deref for Arr<T> {
    type Target = [T];
    #[inline]
    fn deref(&self) -> &[T] {
        match self {
            Arr::Owned(v) => v,
            // 安全：ptr 对齐、指向至少 len 个 T，由 Blob::section 检查过；Blob 比这一列活得久
            Arr::Mapped { ptr, len } => unsafe { std::slice::from_raw_parts(*ptr, *len) },
        }
    }
}

impl<T: Copy> Arr<T> {
    /// 要改的时候换成自己的一份（映射进来的那份只读）。
    pub(crate) fn to_mut(&mut self) -> &mut Vec<T> {
        if let Arr::Mapped { .. } = self {
            *self = Arr::Owned(self.to_vec());
        }
        match self {
            Arr::Owned(v) => v,
            Arr::Mapped { .. } => unreachable!(),
        }
    }

    pub(crate) fn push(&mut self, value: T) {
        self.to_mut().push(value);
    }

    /// 占的堆内存（映射的不算）。
    pub(crate) fn heap_bytes(&self) -> usize {
        match self {
            Arr::Owned(v) => v.capacity() * std::mem::size_of::<T>(),
            Arr::Mapped { .. } => 0,
        }
    }

    /// 收紧容量（建完以后调一次）。
    pub(crate) fn shrink(&mut self) {
        if let Arr::Owned(v) = self {
            v.shrink_to_fit();
        }
    }
}

/// 快照文件的内容：Unix 上 mmap 进来（只读、私有映射），别的平台读进一块 8 字节对齐的堆内存。
pub(crate) struct Blob {
    ptr: *const u8,
    len: usize,
    #[allow(dead_code)]
    heap: Vec<u64>,
    #[cfg_attr(not(unix), allow(dead_code))]
    mapped: bool,
}

unsafe impl Send for Blob {}
unsafe impl Sync for Blob {}

impl Blob {
    /// 复制一份字节（测试、读不了映射的平台用）。
    pub(crate) fn from_bytes(data: &[u8]) -> Blob {
        let mut heap = vec![0u64; (data.len() + 7) / 8];
        // 安全：heap 至少 data.len() 字节
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                heap.as_mut_ptr().cast::<u8>(),
                data.len(),
            );
        }
        Blob {
            ptr: heap.as_ptr().cast::<u8>(),
            len: data.len(),
            heap,
            mapped: false,
        }
    }

    /// 打开快照文件。
    pub(crate) fn open(path: &std::path::Path) -> std::io::Result<Blob> {
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let file = std::fs::File::open(path)?;
            let len = usize::try_from(file.metadata()?.len())
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "快照太大"))?;
            if len == 0 {
                return Ok(Blob::from_bytes(&[]));
            }
            // 安全：只读私有映射；文件关掉以后映射照样有效
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    len,
                    libc::PROT_READ,
                    libc::MAP_PRIVATE,
                    file.as_raw_fd(),
                    0,
                )
            };
            if ptr == libc::MAP_FAILED {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Blob {
                ptr: ptr as *const u8,
                len,
                heap: Vec::new(),
                mapped: true,
            })
        }
        #[cfg(not(unix))]
        {
            Blob::read(path)
        }
    }

    /// 整个文件读进一块 8 字节对齐的堆内存（没有 mmap 的平台用）：直接读进去，不先读一份再复制。
    #[cfg_attr(unix, allow(dead_code))]
    pub(crate) fn read(path: &std::path::Path) -> std::io::Result<Blob> {
        use std::io::Read;
        let mut file = std::fs::File::open(path)?;
        let len = usize::try_from(file.metadata()?.len())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "快照太大"))?;
        let mut heap = vec![0u64; (len + 7) / 8];
        // 安全：heap 至少 len 字节，u64 任何字节都合法
        let bytes = unsafe { std::slice::from_raw_parts_mut(heap.as_mut_ptr().cast::<u8>(), len) };
        file.read_exact(bytes)?;
        Ok(Blob {
            ptr: heap.as_ptr().cast::<u8>(),
            len,
            heap,
            mapped: false,
        })
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        if self.len == 0 {
            return &[];
        }
        // 安全：ptr 指向 len 字节（映射或 heap），活到 Blob 释放
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// `[offset, offset + count * size_of::<T>())` 这一段当成 `T` 的数组。越界、没对齐返回 None。
    ///
    /// 只给 u8/u16/u32/f64 这些任何字节都合法的类型用。
    pub(crate) fn section<T: Copy>(&self, offset: usize, count: usize) -> Option<Arr<T>> {
        let size = std::mem::size_of::<T>();
        let bytes = count.checked_mul(size)?;
        let end = offset.checked_add(bytes)?;
        if end > self.len || offset % std::mem::align_of::<T>() != 0 {
            return None;
        }
        if count == 0 {
            return Some(Arr::Owned(Vec::new()));
        }
        // 基址是页对齐（mmap）或 8 字节对齐（heap），offset 对齐了地址就对齐
        let ptr = unsafe { self.ptr.add(offset) }.cast::<T>();
        if (ptr as usize) % std::mem::align_of::<T>() != 0 {
            return None;
        }
        Some(Arr::Mapped { ptr, len: count })
    }
}

impl Drop for Blob {
    fn drop(&mut self) {
        #[cfg(unix)]
        if self.mapped {
            // 安全：ptr/len 就是 mmap 给的
            unsafe {
                libc::munmap(self.ptr as *mut libc::c_void, self.len);
            }
        }
    }
}

/// 64 位哈希的最后一步搅拌（MurmurHash3 fmix64），开放寻址表的槽位和标签都从这里取。
#[inline]
pub(crate) fn mix(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^= h >> 33;
    h
}

/// 表里一个槽：高 8 位是哈希标签，低 24 位是「编号 + 1」，0 是空槽。
const ID_BITS: u32 = 24;
const ID_MASK: u32 = (1 << ID_BITS) - 1;
/// 能放进表里的最大编号（不含）。
pub(crate) const MAX_IDS: usize = ID_MASK as usize - 1;

/// 至少要多少个槽：负载不超过 0.8，2 的幂。
fn slots_for(n: usize) -> usize {
    (n + n / 4 + 1).next_power_of_two().max(8)
}

/// 编号 -> 键的开放寻址哈希表（线性探测）。键本身不存，比对时由调用方按编号去比。
#[derive(Default)]
pub(crate) struct IdTable {
    pub(crate) slots: Arr<u32>,
    len: usize,
}

impl IdTable {
    /// 能放 n 个编号的空表。
    pub(crate) fn with_capacity(n: usize) -> IdTable {
        IdTable {
            slots: Arr::Owned(vec![0; slots_for(n)]),
            len: 0,
        }
    }

    /// 从快照里读出来的槽：长度是 2 的幂、编号都小于 count、至少一个空槽，不然 None。
    pub(crate) fn from_slots(slots: Arr<u32>, count: usize) -> Option<IdTable> {
        if slots.is_empty() {
            return (count == 0).then(IdTable::default);
        }
        if !slots.len().is_power_of_two() {
            return None;
        }
        let limit = count as u32;
        let (len, bad) = slots.iter().fold((0usize, false), |(len, bad), &s| {
            let id = s & ID_MASK;
            (
                len + usize::from(s != 0),
                bad | ((s != 0) & ((id == 0) | (id > limit))),
            )
        });
        (!bad && len < slots.len() && len == count).then_some(IdTable { slots, len })
    }

    #[inline]
    pub(crate) fn find(&self, hash: u64, mut eq: impl FnMut(u32) -> bool) -> Option<u32> {
        let slots = &*self.slots;
        if slots.is_empty() {
            return None;
        }
        let m = mix(hash);
        let tag = (m >> 56) as u32;
        let mask = slots.len() - 1;
        let mut at = m as usize & mask;
        loop {
            let s = slots[at];
            if s == 0 {
                return None;
            }
            if s >> ID_BITS == tag {
                let id = (s & ID_MASK) - 1;
                if eq(id) {
                    return Some(id);
                }
            }
            at = (at + 1) & mask;
        }
    }

    /// 放进一个（调用方保证还没有、而且槽够）。
    fn put(slots: &mut [u32], hash: u64, id: u32) {
        let m = mix(hash);
        let tag = (m >> 56) as u32;
        let mask = slots.len() - 1;
        let mut at = m as usize & mask;
        while slots[at] != 0 {
            at = (at + 1) & mask;
        }
        slots[at] = (tag << ID_BITS) | (id + 1);
    }

    /// 放进一个编号（调用方保证还没有）；槽不够时按 `rehash`（编号 -> 哈希）整张重排。
    pub(crate) fn insert(&mut self, hash: u64, id: u32, rehash: impl Fn(u32) -> u64) {
        assert!((id as usize) < MAX_IDS, "词库太大");
        if self.slots.is_empty() || (self.len + 1) * 5 > self.slots.len() * 4 {
            let mut slots = vec![0u32; slots_for((self.len + 1) * 2)];
            for &s in self.slots.iter() {
                if s != 0 {
                    let old = (s & ID_MASK) - 1;
                    Self::put(&mut slots, rehash(old), old);
                }
            }
            self.slots = Arr::Owned(slots);
        }
        Self::put(self.slots.to_mut(), hash, id);
        self.len += 1;
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        self.slots.heap_bytes()
    }
}

/// 哈希值的集合（`has_longer` 剪枝用）：存 64 位哈希折成的 32 位指纹。
///
/// 以前是 `HashSet<u64>`。指纹撞了只会让 has_longer 多说一次「有」，和以前哈希撞了一样只影响速度、
/// 不影响结果（93 万词的词库约六千分之一的概率）。
#[derive(Default)]
pub(crate) struct FpSet {
    pub(crate) slots: Arr<u32>,
    len: usize,
}

impl FpSet {
    #[inline]
    fn fingerprint(hash: u64) -> u32 {
        let v = (mix(hash) >> 32) as u32;
        v.max(1)
    }

    pub(crate) fn from_slots(slots: Arr<u32>) -> Option<FpSet> {
        if slots.is_empty() {
            return Some(FpSet::default());
        }
        if !slots.len().is_power_of_two() {
            return None;
        }
        let len = slots.iter().filter(|&&s| s != 0).count();
        (len < slots.len()).then_some(FpSet { slots, len })
    }

    #[inline]
    pub(crate) fn contains(&self, hash: u64) -> bool {
        let slots = &*self.slots;
        if slots.is_empty() {
            return false;
        }
        let v = Self::fingerprint(hash);
        let mask = slots.len() - 1;
        let mut at = v as usize & mask;
        loop {
            let s = slots[at];
            if s == v {
                return true;
            }
            if s == 0 {
                return false;
            }
            at = (at + 1) & mask;
        }
    }

    pub(crate) fn insert(&mut self, hash: u64) {
        let v = Self::fingerprint(hash);
        if self.contains(hash) {
            return;
        }
        if self.slots.is_empty() || (self.len + 1) * 5 > self.slots.len() * 4 {
            let mut slots = vec![0u32; slots_for((self.len + 1) * 2)];
            for &s in self.slots.iter() {
                if s != 0 {
                    Self::put(&mut slots, s);
                }
            }
            self.slots = Arr::Owned(slots);
        }
        Self::put(self.slots.to_mut(), v);
        self.len += 1;
    }

    fn put(slots: &mut [u32], v: u32) {
        let mask = slots.len() - 1;
        let mut at = v as usize & mask;
        while slots[at] != 0 {
            at = (at + 1) & mask;
        }
        slots[at] = v;
    }

    /// 建好以后收紧到合适的大小（插入时为了少重排会留得比较宽）。
    pub(crate) fn compact(&mut self) {
        let want = slots_for(self.len);
        if want < self.slots.len() {
            let mut slots = vec![0u32; want];
            for &s in self.slots.iter() {
                if s != 0 {
                    Self::put(&mut slots, s);
                }
            }
            self.slots = Arr::Owned(slots);
        }
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        self.slots.heap_bytes()
    }
}

/// 一组一段的下标表（CSR）：第 g 组是 `ids[end[g-1]..end[g]]`。建好以后再加的词条放在 `extra`。
#[derive(Default)]
pub(crate) struct Postings {
    pub(crate) end: Arr<u32>,
    pub(crate) ids: Arr<u32>,
    /// 建好以后用 insert 加进来的（用户词库全是这样加的）：组 -> 下标，生僻字排在最后。
    pub(crate) extra: std::collections::HashMap<u32, Vec<u32>>,
}

impl Postings {
    /// 第 g 组建好时就有的那一段。
    #[inline]
    pub(crate) fn frozen(&self, g: u32) -> &[u32] {
        let g = g as usize;
        let Some(&end) = self.end.get(g) else {
            return &[];
        };
        let start = if g == 0 { 0 } else { self.end[g - 1] };
        &self.ids[start as usize..end as usize]
    }

    #[inline]
    pub(crate) fn added(&self, g: u32) -> &[u32] {
        if self.extra.is_empty() {
            return &[];
        }
        self.extra.get(&g).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        self.end.heap_bytes()
            + self.ids.heap_bytes()
            + self
                .extra
                .values()
                .map(|v| v.capacity() * 4 + 48)
                .sum::<usize>()
    }

    /// 从快照读出来的：end 单调不减、最后一个等于 ids 的长度，ids 都小于 n。
    pub(crate) fn check(&self, groups: usize, n: usize) -> bool {
        let increasing = self.end.windows(2).fold(true, |ok, w| ok & (w[0] <= w[1]));
        self.end.len() == groups
            && increasing
            && self.end.last().map_or(0, |&e| e as usize) == self.ids.len()
            && all_below(&self.ids, n)
    }
}

/// 每个数都小于 `n`。
pub(crate) fn all_below(values: &[u32], n: usize) -> bool {
    values.is_empty() || (values.iter().fold(0u32, |m, &v| m.max(v)) as usize) < n
}

/// 严格递增、都大于 0、最后一个是 `last`（空的时候 `last` 要是 0）：一段一段的结束位置，每段不空。
pub(crate) fn strictly_increasing(ends: &[u32], last: usize) -> bool {
    let increasing = ends.windows(2).fold(true, |ok, w| ok & (w[0] < w[1]));
    increasing
        && ends.first().map_or(true, |&e| e > 0)
        && ends.last().map_or(0, |&e| e as usize) == last
}

/// 把 `ids` 稳定地分成「不是生僻字的在前、生僻字在后」。
pub(crate) fn rare_last(ids: &mut [u32], is_rare: impl Fn(u32) -> bool, scratch: &mut Vec<u32>) {
    if !ids.iter().any(|&i| is_rare(i)) {
        return;
    }
    scratch.clear();
    scratch.extend(ids.iter().copied().filter(|&i| !is_rare(i)));
    scratch.extend(ids.iter().copied().filter(|&i| is_rare(i)));
    ids.copy_from_slice(scratch);
}

/// 快照内容的校验和：每 8 字节一个字，四路交错乘加，几十 MB 十来毫秒。
pub(crate) fn checksum(data: &[u8]) -> u64 {
    const K: [u64; 4] = [
        0x9e37_79b9_7f4a_7c15,
        0xc2b2_ae3d_27d4_eb4f,
        0x1656_67b1_9e37_79f9,
        0x27d4_eb2f_1656_67c5,
    ];
    let mut lanes = [1u64, 2, 3, 4];
    let mut chunks = data.chunks_exact(32);
    for chunk in &mut chunks {
        for (lane, (k, word)) in lanes.iter_mut().zip(K.iter().zip(chunk.chunks_exact(8))) {
            let w = u64::from_le_bytes(word.try_into().unwrap());
            *lane = (*lane ^ w).wrapping_mul(*k).rotate_left(29);
        }
    }
    let mut h = data.len() as u64;
    for lane in lanes {
        h = mix(h ^ lane);
    }
    for &b in chunks.remainder() {
        h = (h ^ u64::from(b)).wrapping_mul(0x1000_0000_01b3);
    }
    mix(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_table_finds_what_was_inserted() {
        let keys: Vec<u64> = (0..5000u64).map(|k| k * 7919).collect();
        let mut table = IdTable::default();
        for (id, &k) in keys.iter().enumerate() {
            table.insert(k, id as u32, |i| keys[i as usize]);
        }
        for (id, &k) in keys.iter().enumerate() {
            assert_eq!(table.find(k, |i| keys[i as usize] == k), Some(id as u32));
        }
        assert_eq!(table.find(3, |i| keys[i as usize] == 3), None);
        let reread = IdTable::from_slots(Arr::Owned(table.slots.to_vec()), keys.len()).unwrap();
        assert_eq!(
            reread.find(keys[10], |i| keys[i as usize] == keys[10]),
            Some(10)
        );
        assert!(IdTable::from_slots(Arr::Owned(table.slots.to_vec()), 10).is_none());
    }

    #[test]
    fn fp_set_membership() {
        let mut set = FpSet::default();
        for k in 0..3000u64 {
            set.insert(k * 31);
        }
        set.compact();
        for k in 0..3000u64 {
            assert!(set.contains(k * 31));
        }
        let misses = (0..3000u64).filter(|k| set.contains(k * 31 + 1)).count();
        assert!(misses < 3, "{misses}");
    }

    #[test]
    fn rare_last_is_stable() {
        let mut ids = vec![1, 10, 2, 11, 3];
        rare_last(&mut ids, |i| i >= 10, &mut Vec::new());
        assert_eq!(ids, [1, 2, 3, 10, 11]);
    }

    #[test]
    fn mapped_and_read_blobs_have_the_file() {
        let path = std::env::temp_dir().join(format!("wowtype_blob_{}.bin", std::process::id()));
        let data: Vec<u8> = (0..1000u32).map(|i| (i * 7) as u8).collect();
        std::fs::write(&path, &data).unwrap();
        assert_eq!(Blob::open(&path).unwrap().bytes(), &data[..]);
        assert_eq!(Blob::read(&path).unwrap().bytes(), &data[..]);
        std::fs::write(&path, b"").unwrap();
        assert!(Blob::open(&path).unwrap().bytes().is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn blob_sections_check_bounds_and_alignment() {
        let blob = Blob::from_bytes(&[1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&*blob.section::<u32>(0, 2).unwrap(), &[1, 2]);
        assert!(blob.section::<u32>(4, 2).is_none());
        assert!(blob.section::<u32>(2, 1).is_none());
        assert!(blob.section::<u64>(usize::MAX - 2, 1).is_none());
    }
}
