#[derive(Debug, thiserror::Error)]
pub enum OrderKeyError {
    #[error("lhs {l:?} is less than rhs {r:?}")]
    LhsLess { l: Vec<u32>, r: Vec<u32> },
    #[error("lhs {l:?} is equal to rhs {r:?}")]
    Equal { l: Vec<u32>, r: Vec<u32> },
    #[error("lhs {l:?} is min")]
    Min { l: Vec<u32> },
    #[error("{l:?} is invalid")]
    Invalid { l: Vec<u32> },
}
