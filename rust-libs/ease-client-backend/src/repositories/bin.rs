use redb::TypeName;
use std::fmt::Debug;

#[derive(Debug)]
pub struct BinSerde<T>(T);

impl<T> BinSerde<T> {
    pub fn new(v: T) -> Self {
        Self(v)
    }
    pub fn unwrap(self) -> T {
        self.0
    }
}

impl<T> redb::Value for BinSerde<T>
where
    T: Debug + bitcode::Encode + for<'a> bitcode::Decode<'a>,
{
    type SelfType<'a> = T
    where
        Self: 'a;

    type AsBytes<'a> = Vec<u8>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        bitcode::decode(data).unwrap()
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        bitcode::encode(value)
    }

    fn type_name() -> TypeName {
        TypeName::new(&format!("BinSerde<{}>", std::any::type_name::<T>()))
    }
}

impl<T> redb::Key for BinSerde<T>
where
    T: Debug + bitcode::Encode + bitcode::DecodeOwned + Ord,
{
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        <Self as redb::Value>::from_bytes(data1).cmp(&<Self as redb::Value>::from_bytes(data2))
    }
}
