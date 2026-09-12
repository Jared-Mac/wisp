//! Bounded preflight rejects duplicate keys even inside existing signed types.
//! It deliberately retains no scalar values or error excerpts from private JSON.
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::{cell::Cell, collections::BTreeSet, fmt};
use zeroize::Zeroize;

const MAX_DEPTH: usize = 24;
const MAX_ITEMS: usize = 100_000;
const MAX_CONTAINER: usize = 4096;
const MAX_STRING: usize = 16_384;

struct Scan<'a> {
    depth: usize,
    remaining: &'a Cell<usize>,
}
impl Scan<'_> {
    fn child(&self) -> Scan<'_> {
        Scan {
            depth: self.depth + 1,
            remaining: self.remaining,
        }
    }
}

impl<'de> DeserializeSeed<'de> for Scan<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        if self.depth > MAX_DEPTH || self.remaining.get() == 0 {
            return Err(serde::de::Error::custom("Backup structure exceeds limits"));
        }
        self.remaining.set(self.remaining.get() - 1);
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Scan<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded backup JSON")
    }
    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_none<E: serde::de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<(), E> {
        if value.len() > MAX_STRING {
            return Err(E::custom("Backup string exceeds limits"));
        }
        Ok(())
    }
    fn visit_string<E: serde::de::Error>(self, mut value: String) -> Result<(), E> {
        let result = self.visit_str(&value);
        value.zeroize();
        result
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        let mut count = 0;
        while seq.next_element_seed(self.child())?.is_some() {
            count += 1;
            if count > MAX_CONTAINER {
                return Err(serde::de::Error::custom("Backup collection exceeds limits"));
            }
        }
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        struct Keys(BTreeSet<String>);
        impl Drop for Keys {
            fn drop(&mut self) {
                for mut key in std::mem::take(&mut self.0) {
                    key.zeroize();
                }
            }
        }
        let mut keys = Keys(BTreeSet::new());
        while let Some(mut key) = map.next_key::<String>()? {
            let noncanonical_id = uuid::Uuid::parse_str(&key).is_ok_and(|id| id.to_string() != key);
            if key.len() > MAX_STRING
                || noncanonical_id
                || keys.0.len() >= MAX_CONTAINER
                || keys.0.contains(&key)
            {
                key.zeroize();
                return Err(serde::de::Error::custom(
                    "Duplicate or oversized backup field",
                ));
            }
            keys.0.insert(key);
            map.next_value_seed(self.child())?;
        }
        Ok(())
    }
}

pub(super) fn validate(bytes: &[u8]) -> anyhow::Result<()> {
    let remaining = Cell::new(MAX_ITEMS);
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    Scan {
        depth: 0,
        remaining: &remaining,
    }
    .deserialize(&mut decoder)
    .map_err(|_| anyhow::anyhow!("Invalid or oversized backup JSON"))?;
    decoder
        .end()
        .map_err(|_| anyhow::anyhow!("Invalid backup JSON"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_keys_escaped_aliases_depth_and_trailing_values_fail() {
        for bytes in [
            br#"{"pins":{"a":1,"a":2}}"#.as_slice(),
            br#"{"a":1,"\u0061":2}"#,
            b"{}{}",
        ] {
            assert!(validate(bytes).is_err());
        }
        assert!(validate(format!("{}0{}", "[".repeat(25), "]".repeat(25)).as_bytes()).is_err());
        validate(br#"{"pins":{"a":1,"b":2},"optional":null}"#).unwrap();
    }
}
