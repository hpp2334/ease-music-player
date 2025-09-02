use std::cmp::Ordering;

use crate::OrderKeyError;

const DEFAULT: u32 = u32::MAX / 2;

#[derive(Debug, Clone, Copy)]
pub struct OrderKeyRef<'a> {
    value: &'a [u32],
}

const fn mid(a: u32, b: u32) -> u32 {
    a / 2 + b / 2 + (a & b & 1)
}

impl<'a> PartialEq for OrderKeyRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl<'a> Eq for OrderKeyRef<'a> {}

impl<'a> PartialOrd for OrderKeyRef<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for OrderKeyRef<'a> {
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

impl<'a> OrderKeyRef<'a> {
    pub fn wrap<V: AsRef<[u32]> + ?Sized>(value: &'a V) -> Self {
        Self {
            value: value.as_ref(),
        }
    }

    pub(crate) fn default() -> Vec<u32> {
        vec![DEFAULT]
    }

    pub(crate) fn min() -> Self {
        OrderKeyRef::wrap(&[0])
    }

    fn is_min(&self) -> bool {
        *self == OrderKeyRef::wrap(&[0])
    }

    fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub(crate) fn greater(a: OrderKeyRef) -> Vec<u32> {
        if a.is_empty() {
            return Self::default();
        }

        let last = a.value.last().cloned().unwrap();
        let m = mid(last, u32::MAX);

        if last == m {
            let mut cloned = a.value.to_vec();
            cloned.push(DEFAULT);
            return cloned;
        } else {
            let mut cloned = a.value.to_vec();
            *cloned.last_mut().unwrap() = m;
            return cloned;
        }
    }

    pub(crate) fn less(a: OrderKeyRef) -> Result<Vec<u32>, OrderKeyError> {
        if a.is_min() {
            return Err(OrderKeyError::Min {
                l: a.value.to_vec(),
            });
        }

        Self::between(Self::min(), a)
    }

    pub(crate) fn between(a: OrderKeyRef, b: OrderKeyRef) -> Result<Vec<u32>, OrderKeyError> {
        #[derive(Debug, PartialEq, Eq)]
        enum LeftFill {
            None,
            ContinueLeft,
        }

        let cmp = a.cmp(&b);
        if cmp == Ordering::Greater {
            return Err(OrderKeyError::LhsLess {
                l: a.value.to_vec(),
                r: b.value.to_vec(),
            });
        }

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
            Ok(Self::min().value.to_vec())
        } else {
            Ok(cloned)
        }
    }
}
