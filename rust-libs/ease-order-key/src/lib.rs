use std::u32;

#[derive(Debug, Clone)]
pub struct OrderKey {
    value: Vec<u32>,
}

const DEFAULT: u32 = u32::MAX / 2;

const fn mid(a: u32, b: u32) -> u32 {
    a / 2 + b / 2 + (a & b & 1)
}

impl PartialEq for OrderKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for OrderKey {}

impl PartialOrd for OrderKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let max_len = self.value.len().max(other.value.len());

        for i in 0..max_len {
            let left = self.value.get(i).copied().unwrap_or(0);
            let right = other.value.get(i).copied().unwrap_or(0);
            match left.cmp(&right) {
                std::cmp::Ordering::Equal => continue,
                ord => return ord,
            }
        }
        std::cmp::Ordering::Equal
    }
}

impl OrderKey {
    pub fn wrap(value: Vec<u32>) -> Self {
        Self { value }
    }

    pub fn default() -> Self {
        Self {
            value: vec![DEFAULT],
        }
    }

    fn min() -> Self {
        Self { value: vec![0] }
    }

    fn is_min(&self) -> bool {
        self == &Self::min()
    }

    fn is_valid(&self) -> bool {
        !self.value.is_empty()
    }

    pub fn build_greater(a: &OrderKey) -> Self {
        if !a.is_valid() {
            return Self::default();
        }

        let last = a.value.last().cloned().unwrap();
        let m = mid(last, u32::MAX);

        if last == m {
            let mut cloned = a.value.clone();
            cloned.push(DEFAULT);
            return Self { value: cloned };
        } else {
            let mut cloned = a.value.clone();
            *cloned.last_mut().unwrap() = m;
            return Self { value: cloned };
        }
    }

    pub fn build_less(a: &OrderKey) -> Self {
        if !a.is_valid() || a.is_min() {
            return a.clone();
        }

        Self::build_between(&Self::min(), a)
    }

    pub fn build_between(a: &OrderKey, b: &OrderKey) -> Self {
        #[derive(Debug, PartialEq, Eq)]
        enum LeftFill {
            None,
            ContinueLeft,
        }

        assert!(a <= b);

        let mut fill = LeftFill::None;
        let max_len = a.value.len().max(b.value.len());
        let mut cloned: Vec<u32> = Default::default();
        cloned.reserve(max_len);

        let mut i = 0;
        while i < max_len {
            let left = a.value.get(i).copied().unwrap_or(0);
            let right = b.value.get(i).copied().unwrap_or(0);
            i += 1;
            debug_assert!(left <= right);

            if left == right {
                cloned.push(left);
            } else {
                let m = mid(left, right);
                debug_assert!(m != right);

                cloned.push(m);
                if m == left {
                    fill = LeftFill::ContinueLeft;
                }
                break;
            }
        }
        if fill == LeftFill::ContinueLeft {
            let mut append = true;
            while i < max_len {
                let left = a.value.get(i).copied().unwrap_or(0);
                i += 1;

                let m = mid(left, u32::MAX);
                cloned.push(m);
                if m != left {
                    append = false;
                    break;
                }
            }

            if append {
                cloned.push(u32::MAX / 2);
            }
        }

        while !cloned.is_empty() && cloned.last() == Some(&0) {
            cloned.pop();
        }

        if cloned.is_empty() {
            return Self::min();
        }

        Self { value: cloned }
    }

    pub fn into_raw(self) -> Vec<u32> {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use crate::OrderKey;

    #[test]
    fn test_1() {
        let a = OrderKey::wrap(vec![2]);
        let b = OrderKey::wrap(vec![4]);

        let m = OrderKey::build_between(&a, &b);
        assert_eq!(m, OrderKey::wrap(vec![3]));

        let l = OrderKey::build_less(&a);
        assert_eq!(l, OrderKey::wrap(vec![1]));

        let r = OrderKey::build_greater(&b);
        assert_eq!(r, OrderKey::wrap(vec![((u32::MAX as u64 + 4) / 2) as u32]));
    }

    #[test]
    fn test_2() {
        let a = OrderKey::wrap(vec![2]);
        let b = OrderKey::wrap(vec![2, 1]);

        let m = OrderKey::build_between(&a, &b);
        assert_eq!(m, OrderKey::wrap(vec![2, 0, u32::MAX / 2]));

        let l = OrderKey::build_less(&a);
        assert_eq!(l, OrderKey::wrap(vec![1]));

        let r = OrderKey::build_greater(&b);
        assert_eq!(
            r,
            OrderKey::wrap(vec![2, ((u32::MAX as u64 + 2) / 2) as u32])
        );
    }

    #[test]
    fn test_3() {
        let a = OrderKey::wrap(vec![2]);
        let b = OrderKey::wrap(vec![2, 2]);

        let m = OrderKey::build_between(&a, &b);
        assert_eq!(m, OrderKey::wrap(vec![2, 1]));

        let l = OrderKey::build_less(&a);
        assert_eq!(l, OrderKey::wrap(vec![1]));

        let r = OrderKey::build_greater(&b);
        assert_eq!(
            r,
            OrderKey::wrap(vec![2, ((u32::MAX as u64 + 1) / 2) as u32])
        );
    }

    #[test]
    fn test_4() {
        let a = OrderKey::wrap(vec![2, 2, 4]);
        let b = OrderKey::wrap(vec![2, 3]);

        assert!(a < b);
        let m = OrderKey::build_between(&a, &b);
        assert_eq!(
            m,
            OrderKey::wrap(vec![2, 2, ((u32::MAX as u64 + 4) / 2) as u32])
        );

        let l = OrderKey::build_less(&a);
        assert_eq!(l, OrderKey::wrap(vec![1]));

        let r = OrderKey::build_greater(&b);
        assert_eq!(
            r,
            OrderKey::wrap(vec![2, ((u32::MAX as u64 + 3) / 2) as u32])
        );
    }

    #[test]
    fn test_5() {
        let a = OrderKey::wrap(vec![2, 2, u32::MAX, u32::MAX]);
        let b = OrderKey::wrap(vec![2, 3]);

        let m = OrderKey::build_between(&a, &b);
        assert_eq!(
            m,
            OrderKey::wrap(vec![2, 2, u32::MAX, u32::MAX, u32::MAX / 2])
        );

        let l = OrderKey::build_less(&a);
        assert_eq!(l, OrderKey::wrap(vec![1]));

        let r = OrderKey::build_greater(&b);
        assert_eq!(
            r,
            OrderKey::wrap(vec![2, ((u32::MAX as u64 + 3) / 2) as u32])
        );
    }

    #[test]
    fn test_6() {
        let a = OrderKey::wrap(vec![0]);
        let b = OrderKey::wrap(vec![1]);

        let m = OrderKey::build_between(&a, &b);
        assert_eq!(m, OrderKey::wrap(vec![0, u32::MAX / 2]));

        let l = OrderKey::build_less(&a);
        assert_eq!(l, OrderKey::wrap(vec![0]));

        let r = OrderKey::build_greater(&b);
        assert_eq!(r, OrderKey::wrap(vec![((u32::MAX as u64 + 1) / 2) as u32]));
    }

    #[test]
    fn test_equal_keys() {
        let a = OrderKey::wrap(vec![5, 10]);
        let b = OrderKey::wrap(vec![5, 10]);

        let m = OrderKey::build_between(&a, &b);
        assert!(a <= m && m <= b);
    }

    #[test]
    fn test_max_values() {
        let a = OrderKey::wrap(vec![u32::MAX - 1]);
        let b = OrderKey::wrap(vec![u32::MAX]);

        let m = OrderKey::build_between(&a, &b);
        assert!(a < m && m < b);

        let r = OrderKey::build_greater(&b);
        assert!(b < r);
    }

    #[test]
    fn test_empty_vec_edge_case() {
        let empty = OrderKey::wrap(vec![]);
        assert!(!empty.is_valid());

        let default = OrderKey::default();
        assert!(default.is_valid());
        assert_eq!(default, OrderKey::wrap(vec![u32::MAX / 2]));
    }

    #[test]
    fn test_min_key_behavior() {
        let min_key = OrderKey::min();
        assert!(min_key.is_min());
        assert_eq!(min_key, OrderKey::wrap(vec![0]));

        let less_than_min = OrderKey::build_less(&min_key);
        assert_eq!(less_than_min, min_key); // Should return same key
    }

    #[test]
    fn test_long_sequences() {
        let a = OrderKey::wrap(vec![1, 2, 3, 4, 5]);
        let b = OrderKey::wrap(vec![1, 2, 3, 4, 7]);

        let m = OrderKey::build_between(&a, &b);
        assert!(a < m && m < b);

        let c = OrderKey::wrap(vec![1, 2, 3, 4, 5, u32::MAX]);
        let d = OrderKey::wrap(vec![1, 2, 3, 4, 6]);

        let m2 = OrderKey::build_between(&c, &d);
        assert!(c < m2 && m2 < d);
    }

    #[test]
    fn test_zero_padding() {
        let a = OrderKey::wrap(vec![1, 0, 0]);
        let b = OrderKey::wrap(vec![1, 0, 1]);

        let m = OrderKey::build_between(&a, &b);
        assert!(a < m && m < b);

        // Test that trailing zeros are handled correctly
        let with_trailing_zeros = OrderKey::wrap(vec![2, 0, 0, 0]);
        let without_trailing_zeros = OrderKey::wrap(vec![2]);
        assert_eq!(with_trailing_zeros, without_trailing_zeros);
    }

    #[test]
    fn test_large_gaps() {
        let a = OrderKey::wrap(vec![0]);
        let b = OrderKey::wrap(vec![u32::MAX]);

        let m = OrderKey::build_between(&a, &b);
        assert!(a < m && m < b);
        assert_eq!(m, OrderKey::wrap(vec![u32::MAX / 2]));
    }

    #[test]
    fn test_consecutive_insertions() {
        let mut keys = Vec::new();
        let first = OrderKey::default();
        keys.push(first.clone());

        // Insert 10 keys consecutively
        for _ in 0..10 {
            let last = keys.last().unwrap();
            let next = OrderKey::build_greater(last);
            keys.push(next);
        }

        // Verify all keys are in order
        for i in 1..keys.len() {
            assert!(keys[i - 1] < keys[i]);
        }
    }

    #[test]
    fn test_comparison_operators() {
        let a = OrderKey::wrap(vec![1, 2]);
        let b = OrderKey::wrap(vec![1, 3]);
        let c = OrderKey::wrap(vec![1, 2]);
        let d = OrderKey::wrap(vec![2]);

        assert!(a < b);
        assert!(a == c);
        assert!(a != b);
        assert!(a <= b);
        assert!(a <= c);
        assert!(b >= a);
        assert!(c >= a);
        assert!(a < d);
        assert!(d > a);
    }

    #[test]
    fn test_different_length_comparisons() {
        let short = OrderKey::wrap(vec![5]);
        let medium = OrderKey::wrap(vec![5, 0]);
        let long = OrderKey::wrap(vec![5, 0, 0]);

        // These should all be equal due to padding with zeros
        assert_eq!(short, medium);
        assert_eq!(medium, long);
        assert_eq!(short, long);

        let different = OrderKey::wrap(vec![5, 1]);
        assert!(short < different);
        assert!(medium < different);
        assert!(long < different);
    }

    #[test]
    fn test_into_raw() {
        let raw_vec = vec![1, 2, 3, 4];
        let key = OrderKey::wrap(raw_vec.clone());
        let recovered = key.into_raw();
        assert_eq!(raw_vec, recovered);
    }
}
