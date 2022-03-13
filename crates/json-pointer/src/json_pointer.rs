use std::{
    fmt::{self, Debug, Display, Formatter},
    hash::{Hash, Hasher},
    str::FromStr,
};

use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

use crate::{parser::parse_json_pointer, JsonPointerRef, ParseJsonPointerError, ToJsonPointerRef};

#[derive(Clone, Eq)]
pub struct JsonPointer(pub(crate) Vec<String>);

impl ToJsonPointerRef for JsonPointer {
    fn to_json_pointer_ref(&self) -> JsonPointerRef<'_> {
        self.as_ref()
    }
}

impl<'a> ToJsonPointerRef for &'a JsonPointer {
    fn to_json_pointer_ref(&self) -> JsonPointerRef<'a> {
        self.as_ref()
    }
}

impl PartialEq<JsonPointerRef<'_>> for JsonPointer {
    fn eq(&self, other: &JsonPointerRef<'_>) -> bool {
        self.0.iter().eq(other.iter())
    }
}

impl PartialEq for JsonPointer {
    fn eq(&self, other: &Self) -> bool {
        self.0.iter().eq(other.0.iter())
    }
}

impl Hash for JsonPointer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for segment in &self.0 {
            segment.hash(state);
        }
    }
}

impl Display for JsonPointer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.as_ref(), f)
    }
}

impl Debug for JsonPointer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl FromStr for JsonPointer {
    type Err = ParseJsonPointerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_json_pointer(s).map(Self)
    }
}

impl Serialize for JsonPointer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for JsonPointer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        parse_json_pointer(&String::deserialize(deserializer)?)
            .map(Self)
            .map_err(|err| D::Error::custom(err.to_string()))
    }
}

impl JsonPointer {
    #[inline]
    pub fn root() -> JsonPointer {
        JsonPointer(Vec::new())
    }

    #[inline]
    pub fn as_ref(&self) -> JsonPointerRef<'_> {
        JsonPointerRef {
            prefix: None,
            path: &self.0,
        }
    }

    #[inline]
    pub fn with_prefix<'a>(&'a self, prefix: &'a JsonPointer) -> JsonPointerRef<'a> {
        self.with_prefix_opt(Some(prefix))
    }

    #[inline]
    pub fn with_prefix_opt<'a>(&'a self, prefix: Option<&'a JsonPointer>) -> JsonPointerRef<'a> {
        JsonPointerRef {
            prefix: prefix.map(|prefix| &*prefix.0),
            path: &self.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_ref() {
        let pointer: JsonPointer = "/a/b/c".parse().unwrap();
        let pointer_ref = pointer.as_ref();

        assert_eq!(pointer_ref.to_string(), "/a/b/c");

        let (parent, key) = pointer_ref.split_last().unwrap();
        assert_eq!(parent.to_string(), "/a/b");
        assert_eq!(key, "c");
    }

    #[test]
    fn with_prefix() {
        let prefix: JsonPointer = "/a/b/c".parse().unwrap();
        let pointer: JsonPointer = "/d/e/f".parse().unwrap();
        let pointer_ref = pointer.with_prefix(&prefix);

        assert_eq!(pointer_ref.to_string(), "/a/b/c/d/e/f");

        let (parent, key) = pointer_ref.split_last().unwrap();
        assert_eq!(parent.to_string(), "/a/b/c/d/e");
        assert_eq!(key, "f");

        let (parent, key) = parent.split_last().unwrap();
        assert_eq!(parent.to_string(), "/a/b/c/d");
        assert_eq!(key, "e");

        let (parent, key) = parent.split_last().unwrap();
        assert_eq!(parent.to_string(), "/a/b/c");
        assert_eq!(key, "d");

        let (parent, key) = parent.split_last().unwrap();
        assert_eq!(parent.to_string(), "/a/b");
        assert_eq!(key, "c");
    }
}
