#[macro_export]
macro_rules! define_id {
    ($s:ident) => {
        #[derive(
            Debug,
            Clone,
            Hash,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Copy,
            bitcode::Encode,
            bitcode::Decode,
            uniffi::Record,
        )]
        pub struct $s {
            value: i64,
        }
        impl $s {
            pub fn wrap(value: i64) -> Self {
                Self { value }
            }
        }

        impl AsRef<i64> for $s {
            fn as_ref(&self) -> &i64 {
                &self.value
            }
        }

        impl serde::Serialize for $s {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.value.serialize(serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for $s {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                i64::deserialize(deserializer).map(|p| Self { value: p })
            }
        }
    };
}
