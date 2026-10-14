//! Strict JSON parsing shared by Story and save trust boundaries.
use serde::de::DeserializeOwned;

/// Rejects duplicate object keys before deserializing typed data.
pub fn from_str<T: DeserializeOwned>(text: &str) -> serde_json::Result<T> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    <UniqueKeys as serde::Deserialize>::deserialize(&mut deserializer)?;
    deserializer.end()?;
    serde_json::from_str(text)
}

// A validation pass preserves serde_json's original line/column diagnostics for duplicate nested keys.
struct UniqueKeys;
impl<'de> serde::Deserialize<'de> for UniqueKeys {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = UniqueKeys;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON value with unique object keys")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<UniqueKeys, M::Error> {
                let mut keys = std::collections::BTreeSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !keys.insert(key.clone()) {
                        return Err(serde::de::Error::custom(format!(
                            "duplicate ID or field `{key}`"
                        )));
                    }
                    map.next_value::<UniqueKeys>()?;
                }
                Ok(UniqueKeys)
            }
            fn visit_seq<S: serde::de::SeqAccess<'de>>(
                self,
                mut seq: S,
            ) -> Result<UniqueKeys, S::Error> {
                while seq.next_element::<UniqueKeys>()?.is_some() {}
                Ok(UniqueKeys)
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}
