use std::fmt::{self, Debug, Display, Formatter};

use crate::JsonPointer;

pub trait ToJsonPointerRef {
    fn to_json_pointer_ref(&self) -> JsonPointerRef<'_>;
}

#[derive(Copy, Clone)]
pub struct JsonPointerRef<'a> {
    pub(crate) prefix: Option<&'a [String]>,
    pub(crate) path: &'a [String],
}

impl<'a> ToJsonPointerRef for JsonPointerRef<'a> {
    fn to_json_pointer_ref(&self) -> JsonPointerRef<'a> {
        *self
    }
}

impl PartialEq<JsonPointer> for JsonPointerRef<'_> {
    fn eq(&self, other: &JsonPointer) -> bool {
        self.iter().eq(other.0.iter())
    }
}

impl PartialEq for JsonPointerRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl Eq for JsonPointerRef<'_> {}

impl Display for JsonPointerRef<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for segment in self.iter() {
            f.write_str("/")?;
            f.write_str(segment)?;
        }

        Ok(())
    }
}

impl Debug for JsonPointerRef<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl<'a> JsonPointerRef<'a> {
    pub fn to_owned(&self) -> JsonPointer {
        JsonPointer(self.iter().cloned().collect())
    }

    pub fn split_last(&self) -> Option<(JsonPointerRef<'a>, &'a str)> {
        if let Some((key, parent)) = self.path.split_last() {
            return Some((
                JsonPointerRef {
                    prefix: self.prefix,
                    path: parent,
                },
                key,
            ));
        }

        self.prefix
            .as_ref()
            .and_then(|path| path.split_last())
            .map(|(key, parent)| {
                (
                    JsonPointerRef {
                        prefix: None,
                        path: parent,
                    },
                    key.as_str(),
                )
            })
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.prefix
            .into_iter()
            .flat_map(|path| path.iter())
            .chain(self.path.iter())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.prefix
            .map(|segments| segments.len())
            .unwrap_or_default()
            + self.path.len()
    }

    pub fn starts_with(&self, needle: JsonPointerRef<'_>) -> bool {
        let prefix_len = needle.len();
        if prefix_len > self.len() {
            return false;
        }
        if prefix_len == 0 {
            return true;
        }

        let count = self
            .iter()
            .zip(needle.iter())
            .enumerate()
            .take_while(|(_, (a, b))| a == b)
            .count();
        count == needle.len()
    }

    pub fn strip_prefix(&self, prefix: JsonPointerRef<'_>) -> Option<JsonPointerRef<'a>> {
        let prefix_len = prefix.len();
        if prefix_len > self.len() {
            return None;
        }
        if prefix_len == 0 {
            return Some(*self);
        }

        let mut count = self
            .iter()
            .zip(prefix.iter())
            .enumerate()
            .take_while(|(_, (a, b))| a == b)
            .count();
        if count != prefix.len() {
            return None;
        }

        let prefix = if let Some(prefix) = self.prefix {
            let strip_len = prefix.len().min(count);
            count -= strip_len;
            let new_prefix = &prefix[strip_len..];
            if !new_prefix.is_empty() {
                Some(new_prefix)
            } else {
                None
            }
        } else {
            None
        };
        let path = &self.path[count..];
        Some(JsonPointerRef { prefix, path })
    }
}

#[cfg(test)]
mod tests {
    use crate::json_pointer;

    #[test]
    fn test_starts_with() {
        assert!(json_pointer!("/a/b/c")
            .as_ref()
            .starts_with(json_pointer!("/a/b").as_ref()));

        assert!(json_pointer!("/a/b/c")
            .as_ref()
            .starts_with(json_pointer!("/a/b/c").as_ref()));

        assert!(json_pointer!("/c")
            .with_prefix(&json_pointer!("/a/b"))
            .starts_with(json_pointer!("/a/b").as_ref()));

        assert!(json_pointer!("/c")
            .with_prefix(&json_pointer!("/a/b"))
            .starts_with(json_pointer!("/a/b/c").as_ref()));

        assert!(!json_pointer!("/c")
            .with_prefix(&json_pointer!("/a1/b"))
            .starts_with(json_pointer!("/a/b").as_ref()));

        assert!(!json_pointer!("/c")
            .with_prefix(&json_pointer!("/a/b1"))
            .starts_with(json_pointer!("/a/b").as_ref()));

        assert!(json_pointer!("/c/d/e")
            .with_prefix(&json_pointer!("/a/b"))
            .starts_with(json_pointer!("/a/b/c/d/e").as_ref()));

        assert!(json_pointer!("/c/d/e")
            .with_prefix(&json_pointer!("/a/b"))
            .starts_with(json_pointer!("/a").as_ref()));

        assert!(json_pointer!("/c/d/e")
            .as_ref()
            .starts_with(json_pointer!("").as_ref()));
    }

    #[test]
    fn test_strip_prefix() {
        assert_eq!(
            json_pointer!("/a/b/c")
                .as_ref()
                .strip_prefix(json_pointer!("/a/b").as_ref())
                .unwrap(),
            json_pointer!("/c")
        );

        assert_eq!(
            json_pointer!("/a/b/c")
                .as_ref()
                .strip_prefix(json_pointer!("/a/b/c").as_ref())
                .unwrap(),
            json_pointer!("")
        );

        assert_eq!(
            json_pointer!("/c")
                .with_prefix(&json_pointer!("/a/b"))
                .strip_prefix(json_pointer!("/a/b").as_ref())
                .unwrap(),
            json_pointer!("/c")
        );

        assert_eq!(
            json_pointer!("/c")
                .with_prefix(&json_pointer!("/a/b"))
                .strip_prefix(json_pointer!("/a/b/c").as_ref())
                .unwrap(),
            json_pointer!("")
        );

        assert!(json_pointer!("/c")
            .with_prefix(&json_pointer!("/a1/b"))
            .strip_prefix(json_pointer!("/a/b").as_ref())
            .is_none());

        assert!(json_pointer!("/c")
            .with_prefix(&json_pointer!("/a/b1"))
            .strip_prefix(json_pointer!("/a/b").as_ref())
            .is_none());

        assert_eq!(
            json_pointer!("/c/d/e")
                .with_prefix(&json_pointer!("/a/b"))
                .strip_prefix(json_pointer!("/a/b/c/d/e").as_ref())
                .unwrap(),
            json_pointer!("")
        );

        assert_eq!(
            json_pointer!("/c/d/e")
                .with_prefix(&json_pointer!("/a/b"))
                .strip_prefix(json_pointer!("/a").as_ref())
                .unwrap(),
            json_pointer!("/b/c/d/e")
        );

        assert_eq!(
            json_pointer!("/a/b/c")
                .as_ref()
                .strip_prefix(json_pointer!("").as_ref())
                .unwrap(),
            json_pointer!("/a/b/c")
        );
    }
}
