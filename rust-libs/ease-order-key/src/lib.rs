use std::u32;

pub use crate::error::OrderKeyError;
pub use crate::ipl::OrderKeyRef;

mod error;
mod ipl;

#[derive(Debug, Clone)]
pub struct OrderKey {
    value: Vec<u32>,
}

impl PartialEq for OrderKey {
    fn eq(&self, other: &Self) -> bool {
        let a: OrderKeyRef = self.into();
        let b: OrderKeyRef = other.into();
        a.cmp(&b) == std::cmp::Ordering::Equal
    }
}

impl Eq for OrderKey {}

impl PartialOrd for OrderKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let a: OrderKeyRef = self.into();
        let b: OrderKeyRef = other.into();
        Some(a.cmp(&b))
    }
}

impl Ord for OrderKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let a: OrderKeyRef = self.into();
        let b: OrderKeyRef = other.into();
        a.cmp(&b)
    }
}

impl Default for OrderKey {
    fn default() -> Self {
        Self {
            value: OrderKeyRef::default(),
        }
    }
}

impl<'a> From<&'a OrderKey> for OrderKeyRef<'a> {
    fn from(value: &'a OrderKey) -> Self {
        Self::wrap(value.value.as_slice())
    }
}

impl OrderKey {
    pub fn wrap(value: Vec<u32>) -> Self {
        Self { value }
    }

    #[allow(dead_code)]
    fn min() -> Self {
        Self { value: vec![0] }
    }

    pub fn greater<'a>(a: impl Into<OrderKeyRef<'a>>) -> Self {
        Self {
            value: OrderKeyRef::greater(a.into()),
        }
    }

    pub fn less<'a>(a: impl Into<OrderKeyRef<'a>>) -> Result<Self, OrderKeyError> {
        let res = OrderKeyRef::less(a.into())?;
        Ok(Self { value: res })
    }

    pub fn less_or_fallback<'a>(a: impl Into<OrderKeyRef<'a>>) -> Self {
        let res = OrderKeyRef::less(a.into());
        match res {
            Ok(value) => Self { value },
            Err(_) => Self { value: vec![0] },
        }
    }

    pub fn between<'a>(
        a: impl Into<OrderKeyRef<'a>>,
        b: impl Into<OrderKeyRef<'a>>,
    ) -> Result<Self, OrderKeyError> {
        let res = OrderKeyRef::between(a.into(), b.into())?;
        Ok(Self { value: res })
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

        let m = OrderKey::between(&a, &b).unwrap();
        assert_eq!(m, OrderKey::wrap(vec![3]));

        let l = OrderKey::less_or_fallback(&a);
        assert_eq!(l, OrderKey::wrap(vec![1]));

        let r = OrderKey::greater(&b);
        assert_eq!(r, OrderKey::wrap(vec![((u32::MAX as u64 + 4) / 2) as u32]));
    }

    #[test]
    fn test_2() {
        let a = OrderKey::wrap(vec![2]);
        let b = OrderKey::wrap(vec![2, 1]);

        let m = OrderKey::between(&a, &b).unwrap();
        assert_eq!(m, OrderKey::wrap(vec![2, 0, u32::MAX / 2]));

        let l = OrderKey::less_or_fallback(&a);
        assert_eq!(l, OrderKey::wrap(vec![1]));

        let r = OrderKey::greater(&b);
        assert_eq!(
            r,
            OrderKey::wrap(vec![2, ((u32::MAX as u64 + 2) / 2) as u32])
        );
    }

    #[test]
    fn test_3() {
        let a = OrderKey::wrap(vec![2]);
        let b = OrderKey::wrap(vec![2, 2]);

        let m = OrderKey::between(&a, &b).unwrap();
        assert_eq!(m, OrderKey::wrap(vec![2, 1]));

        let l = OrderKey::less_or_fallback(&a);
        assert_eq!(l, OrderKey::wrap(vec![1]));

        let r = OrderKey::greater(&b);
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
        let m = OrderKey::between(&a, &b).unwrap();
        assert_eq!(
            m,
            OrderKey::wrap(vec![2, 2, ((u32::MAX as u64 + 4) / 2) as u32])
        );

        let l = OrderKey::less_or_fallback(&a);
        assert_eq!(l, OrderKey::wrap(vec![1]));

        let r = OrderKey::greater(&b);
        assert_eq!(
            r,
            OrderKey::wrap(vec![2, ((u32::MAX as u64 + 3) / 2) as u32])
        );
    }

    #[test]
    fn test_5() {
        let a = OrderKey::wrap(vec![2, 2, u32::MAX, u32::MAX]);
        let b = OrderKey::wrap(vec![2, 3]);

        let m = OrderKey::between(&a, &b).unwrap();
        assert_eq!(
            m,
            OrderKey::wrap(vec![2, 2, u32::MAX, u32::MAX, u32::MAX / 2])
        );

        let l = OrderKey::less_or_fallback(&a);
        assert_eq!(l, OrderKey::wrap(vec![1]));

        let r = OrderKey::greater(&b);
        assert_eq!(
            r,
            OrderKey::wrap(vec![2, ((u32::MAX as u64 + 3) / 2) as u32])
        );
    }

    #[test]
    fn test_6() {
        let a = OrderKey::wrap(vec![0]);
        let b = OrderKey::wrap(vec![1]);

        let m = OrderKey::between(&a, &b).unwrap();
        assert_eq!(m, OrderKey::wrap(vec![0, u32::MAX / 2]));

        let l = OrderKey::less_or_fallback(&a);
        assert_eq!(l, OrderKey::wrap(vec![0]));

        let r = OrderKey::greater(&b);
        assert_eq!(r, OrderKey::wrap(vec![((u32::MAX as u64 + 1) / 2) as u32]));
    }

    #[test]
    fn test_equal_keys() {
        let a = OrderKey::wrap(vec![5, 10]);
        let b = OrderKey::wrap(vec![5, 10]);

        let m = OrderKey::between(&a, &b).unwrap();
        assert!(a <= m && m <= b);
    }

    #[test]
    fn test_max_values() {
        let a = OrderKey::wrap(vec![u32::MAX - 1]);
        let b = OrderKey::wrap(vec![u32::MAX]);

        let m = OrderKey::between(&a, &b).unwrap();
        assert!(a < m && m < b);

        let r = OrderKey::greater(&b);
        assert!(b < r);
    }

    #[test]
    fn test_min_key_behavior() {
        let min_key = OrderKey::min();
        assert_eq!(min_key, OrderKey::wrap(vec![0]));

        let less_than_min = OrderKey::less_or_fallback(&min_key);
        assert_eq!(less_than_min, min_key); // Should return same key
    }

    #[test]
    fn test_long_sequences() {
        let a = OrderKey::wrap(vec![1, 2, 3, 4, 5]);
        let b = OrderKey::wrap(vec![1, 2, 3, 4, 7]);

        let m = OrderKey::between(&a, &b).unwrap();
        assert!(a < m && m < b);

        let c = OrderKey::wrap(vec![1, 2, 3, 4, 5, u32::MAX]);
        let d = OrderKey::wrap(vec![1, 2, 3, 4, 6]);

        let m2 = OrderKey::between(&c, &d).unwrap();
        assert!(c < m2 && m2 < d);
    }

    #[test]
    fn test_zero_padding() {
        let a = OrderKey::wrap(vec![1, 0, 0]);
        let b = OrderKey::wrap(vec![1, 0, 1]);

        let m = OrderKey::between(&a, &b).unwrap();
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

        let m = OrderKey::between(&a, &b).unwrap();
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
            let next = OrderKey::greater(last);
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
