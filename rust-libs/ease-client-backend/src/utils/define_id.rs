#[macro_export]
macro_rules! define_id {
    ($s:ident) => {
        #[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Copy)]
        pub struct $s(pub i64);
        impl AsRef<i64> for $s {
            fn as_ref(&self) -> &i64 {
                &self.0
            }
        }

        impl ease_database::ToSql for $s {
            fn to_sql(&self) -> ease_database::Result<ease_database::ToSqlOutput<'_>> {
                Ok(ease_database::ToSqlOutput::Owned(
                    ease_database::Value::Integer(self.0),
                ))
            }
        }

        impl serde::Serialize for $s {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.0.serialize(serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for $s {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                i64::deserialize(deserializer).map(|p| Self(p))
            }
        }
    };
}
