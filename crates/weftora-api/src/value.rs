use crate::StateError;
use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeSeed},
};
use std::{collections::BTreeMap, fmt};

pub const MAX_COLLECTION_ENTRIES: usize = 256;
pub const MAX_COLLECTION_BYTES: usize = 16 * 1024;
pub const MAX_COLLECTION_KEY_BYTES: usize = 512;
pub const MAX_COLLECTION_STRING_BYTES: usize = 16 * 1024;

/// Canonical scalars and flat collections of scalars; map keys serialize in sorted order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Map(BTreeMap<String, Value>),
}

impl Value {
    pub fn float(value: f64) -> Result<Self, StateError> {
        let value = Self::Float(value);
        value.validate()?;
        Ok(value)
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::List(_) => "list",
            Self::Map(_) => "map",
        }
    }

    pub fn is_collection(&self) -> bool {
        matches!(self, Self::List(_) | Self::Map(_))
    }

    /// Checks values constructed by Rust callers as well as deserialized data.
    pub fn validate(&self) -> Result<(), StateError> {
        match self {
            Self::Float(f) if !f.is_finite() => return Err(StateError::NonFinite),
            Self::List(values) => {
                collection_size(values.len(), self.payload_bytes())?;
                for value in values {
                    collection_entry(value)?;
                }
            }
            Self::Map(values) => {
                collection_size(values.len(), self.payload_bytes())?;
                for (key, value) in values {
                    collection_key(key)?;
                    collection_entry(value)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Retained payload accounting: scalar strings, plus collection entries/keys and strings.
    /// Collection entries charge eight bytes each in addition to their variable-length payload.
    pub fn payload_bytes(&self) -> usize {
        match self {
            Self::String(text) => text.len(),
            Self::List(values) => values
                .iter()
                .fold(0usize, |n, v| n.saturating_add(8 + scalar_bytes(v))),
            Self::Map(values) => values.iter().fold(0usize, |n, (k, v)| {
                n.saturating_add(8 + k.len() + scalar_bytes(v))
            }),
            _ => 0,
        }
    }

    /// Save eligibility compares float bits, including signed zero inside collections.
    pub fn same_bits(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Float(a), Self::Float(b)) => a.to_bits() == b.to_bits(),
            (Self::List(a), Self::List(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.same_bits(b))
            }
            (Self::Map(a), Self::Map(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|((ka, a), (kb, b))| ka == kb && a.same_bits(b))
            }
            _ => self == other,
        }
    }
}
fn scalar_bytes(value: &Value) -> usize {
    match value {
        Value::String(text) => text.len(),
        _ => 0,
    }
}
fn collection_entry(value: &Value) -> Result<(), StateError> {
    if value.is_collection() {
        return Err(StateError::InvalidValue(
            "nested collections are unsupported; entries must be scalars".into(),
        ));
    }
    value.validate()?;
    if scalar_bytes(value) > MAX_COLLECTION_STRING_BYTES {
        return Err(StateError::InvalidValue(
            "collection strings exceed 16384 bytes".into(),
        ));
    }
    Ok(())
}
fn collection_key(key: &str) -> Result<(), StateError> {
    if key.len() > MAX_COLLECTION_KEY_BYTES {
        return Err(StateError::InvalidValue("map key exceeds 512 bytes".into()));
    }
    Ok(())
}
fn collection_size(entries: usize, bytes: usize) -> Result<(), StateError> {
    if entries > MAX_COLLECTION_ENTRIES || bytes > MAX_COLLECTION_BYTES {
        return Err(StateError::InvalidValue(
            "collection exceeds 256 entries or 16384 payload bytes".into(),
        ));
    }
    Ok(())
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor { collections: true })
    }
}
struct ScalarSeed;
impl<'de> DeserializeSeed<'de> for ScalarSeed {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(ValueVisitor { collections: false })
    }
}
struct ValueVisitor {
    collections: bool,
}
impl<'de> de::Visitor<'de> for ValueVisitor {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a scalar or a flat scalar list/map")
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Int(value))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(i64::try_from(value)
            .map(Value::Int)
            .unwrap_or_else(|_| Value::Float(value as f64)))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Value::float(value).map_err(E::custom)
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        self.visit_string(value.into())
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        if !self.collections && value.len() > MAX_COLLECTION_STRING_BYTES {
            return Err(E::custom("collection string exceeds 16384 bytes"));
        }
        Ok(Value::String(value))
    }
    fn visit_seq<S: de::SeqAccess<'de>>(self, mut seq: S) -> Result<Value, S::Error> {
        if !self.collections {
            return Err(de::Error::custom("nested collections are unsupported"));
        }
        let mut values = Vec::new();
        let mut bytes = 0usize;
        while let Some(value) = seq.next_element_seed(ScalarSeed)? {
            bytes = bytes.saturating_add(8 + scalar_bytes(&value));
            collection_size(values.len() + 1, bytes).map_err(de::Error::custom)?;
            values.push(value);
        }
        Ok(Value::List(values))
    }
    fn visit_map<M: de::MapAccess<'de>>(self, mut map: M) -> Result<Value, M::Error> {
        if !self.collections {
            return Err(de::Error::custom("nested collections are unsupported"));
        }
        let mut values = BTreeMap::new();
        let mut bytes = 0usize;
        while let Some(key) = map.next_key::<String>()? {
            collection_key(&key).map_err(de::Error::custom)?;
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate map key `{key}`")));
            }
            let value = map.next_value_seed(ScalarSeed)?;
            bytes = bytes.saturating_add(8 + key.len() + scalar_bytes(&value));
            collection_size(values.len() + 1, bytes).map_err(de::Error::custom)?;
            values.insert(key, value);
        }
        Ok(Value::Map(values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, to_string};

    #[test]
    fn scalar_encoding_stays_compatible_and_map_order_is_stable() {
        for (text, expected) in [
            ("true", Value::Bool(true)),
            ("1", Value::Int(1)),
            ("1.0", Value::Float(1.0)),
            ("9223372036854775808", Value::Float(9223372036854775808.0)),
            ("\"text\"", Value::String("text".into())),
        ] {
            assert_eq!(from_str::<Value>(text).unwrap(), expected);
        }
        let a: Value = from_str(r#"{"z":2,"a":1.0,"é":"ok"}"#).unwrap();
        let b: Value = from_str(r#"{"é":"ok","a":1.0,"z":2}"#).unwrap();
        assert_eq!(to_string(&a).unwrap(), to_string(&b).unwrap());
        assert_eq!(to_string(&a).unwrap(), r#"{"a":1.0,"z":2,"é":"ok"}"#);
        for text in [
            "[]",
            "{}",
            "[true,1,1.0,\"x\"]",
            r#"{"":false,"any key":3}"#,
        ] {
            let value: Value = from_str(text).unwrap();
            assert_eq!(
                from_str::<Value>(&to_string(&value).unwrap()).unwrap(),
                value
            );
        }
    }
    #[test]
    fn collection_decoding_is_flat_bounded_and_rejects_duplicates() {
        for bad in ["null", "[null]", "[[1]]", r#"{"x":{}}"#, r#"{"x":1,"x":2}"#] {
            assert!(from_str::<Value>(bad).is_err(), "{bad}");
        }
        let list = Value::List(vec![Value::Bool(true); MAX_COLLECTION_ENTRIES]);
        assert!(from_str::<Value>(&to_string(&list).unwrap()).is_ok());
        let list = Value::List(vec![Value::Bool(true); MAX_COLLECTION_ENTRIES + 1]);
        assert!(list.validate().is_err());
        assert!(from_str::<Value>(&to_string(&list).unwrap()).is_err());
        let exact = Value::List(vec![Value::String("x".repeat(MAX_COLLECTION_BYTES - 8))]);
        assert_eq!(exact.payload_bytes(), MAX_COLLECTION_BYTES);
        assert!(from_str::<Value>(&to_string(&exact).unwrap()).is_ok());
        let over = Value::List(vec![Value::String("x".repeat(MAX_COLLECTION_BYTES - 7))]);
        assert!(over.validate().is_err());
        assert!(from_str::<Value>(&to_string(&over).unwrap()).is_err());
        let long_key = Value::Map(BTreeMap::from([("x".repeat(513), Value::Int(1))]));
        assert!(long_key.validate().is_err());
        assert!(from_str::<Value>(&to_string(&long_key).unwrap()).is_err());
    }
    #[test]
    fn rust_values_are_validated_and_snapshot_comparison_preserves_float_bits() {
        for value in [
            Value::List(vec![Value::Float(f64::NAN)]),
            Value::Map(BTreeMap::from([("x".into(), Value::Float(f64::INFINITY))])),
            Value::List(vec![Value::List(vec![])]),
        ] {
            assert!(value.validate().is_err());
        }
        let negative = Value::Map(BTreeMap::from([("zero".into(), Value::Float(-0.0))]));
        let positive = Value::Map(BTreeMap::from([("zero".into(), Value::Float(0.0))]));
        assert_eq!(negative, positive);
        assert!(!negative.same_bits(&positive));
        assert!(negative.same_bits(&negative.clone()));
        assert!(!Value::List(vec![Value::Int(1)]).same_bits(&Value::List(vec![Value::Float(1.0)])));
    }
}
