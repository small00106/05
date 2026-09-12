//! 固定长度位图：时间槽占用标记的基本数据结构。
//!
//! 每个教师 / 班级 / 教室各持有一个位图，第 `i` 位表示时间槽 `i` 是否被占用。
//! 冲突检测就是一次置位操作（[`Bitmap::set`] 返回旧值），避免 `HashMap` 的哈希开销。

/// 定长位图，内部以 64 位字数组存储。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    words: Vec<u64>,
    len: usize,
}

impl Bitmap {
    /// 创建 `len` 位的全零位图。
    pub fn new(len: usize) -> Self {
        Bitmap {
            words: vec![0; len.div_ceil(64)],
            len,
        }
    }

    /// 位图位数。
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 查询第 `i` 位。`i` 越界会 panic（调用方保证下标合法）。
    #[inline]
    pub fn get(&self, i: usize) -> bool {
        (self.words[i / 64] >> (i % 64)) & 1 == 1
    }

    /// 置位第 `i` 位，返回置位前的值。
    ///
    /// 占用检测的惯用法：`if busy.set(slot) { /* 该槽已被占用，冲突 */ }`
    #[inline]
    pub fn set(&mut self, i: usize) -> bool {
        let word = &mut self.words[i / 64];
        let mask = 1u64 << (i % 64);
        let prev = *word & mask != 0;
        *word |= mask;
        prev
    }

    /// 清除第 `i` 位。
    #[inline]
    pub fn clear(&mut self, i: usize) {
        self.words[i / 64] &= !(1u64 << (i % 64));
    }

    /// 置位总数。
    pub fn count_ones(&self) -> u32 {
        self.words.iter().map(|w| w.count_ones()).sum()
    }

    /// 两位图是否存在同为 1 的位（长度须一致）。
    pub fn intersects(&self, other: &Bitmap) -> bool {
        debug_assert_eq!(self.len, other.len);
        self.words.iter().zip(&other.words).any(|(a, b)| a & b != 0)
    }

    /// 所有置位下标，升序。
    pub fn iter_ones(&self) -> impl Iterator<Item = usize> + '_ {
        let len = self.len;
        self.words
            .iter()
            .enumerate()
            .flat_map(move |(wi, &w)| {
                (0..64).filter_map(move |b| ((w >> b) & 1 == 1).then_some(wi * 64 + b))
            })
            .filter(move |&i| i < len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_clear() {
        let mut b = Bitmap::new(130);
        assert!(!b.get(0));
        assert!(!b.set(5)); // 之前未置位
        assert!(b.get(5));
        assert!(b.set(5)); // 重复置位返回 true
        b.set(129);
        assert!(b.get(129));
        assert!(!b.get(128));
        b.clear(5);
        assert!(!b.get(5));
        assert_eq!(b.count_ones(), 1);
    }

    #[test]
    fn iter_ones_yields_sorted_indices() {
        let mut b = Bitmap::new(70);
        b.set(69);
        b.set(3);
        b.set(64);
        let got: Vec<usize> = b.iter_ones().collect();
        assert_eq!(got, vec![3, 64, 69]);
    }

    #[test]
    fn intersects_works() {
        let mut a = Bitmap::new(40);
        let mut b = Bitmap::new(40);
        a.set(7);
        assert!(!a.intersects(&b));
        b.set(7);
        assert!(a.intersects(&b));
    }
}
