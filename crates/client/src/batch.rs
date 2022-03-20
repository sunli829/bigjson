use serde::Serialize;

use crate::{json_patch::JsonPatch, BigJsonClientError};

pub struct Batch {
    pub(crate) res: Result<Vec<JsonPatch>, BigJsonClientError>,
}

impl Batch {
    pub fn new() -> Self {
        Self { res: Ok(vec![]) }
    }

    pub fn add<T: Serialize>(mut self, path: impl Into<String>, value: &T) -> Self {
        self.res = self.res.and_then(|mut patch_list| {
            patch_list.push(JsonPatch::Add {
                path: path.into(),
                value: serde_json::to_value(value)?,
            });
            Ok(patch_list)
        });
        self
    }

    pub fn replace<T: Serialize>(mut self, path: impl Into<String>, value: &T) -> Self {
        self.res = self.res.and_then(|mut patch_list| {
            patch_list.push(JsonPatch::Replace {
                path: path.into(),
                value: serde_json::to_value(value)?,
            });
            Ok(patch_list)
        });
        self
    }

    pub fn remove(mut self, path: impl Into<String>) -> Self {
        self.res = self.res.and_then(|mut patch_list| {
            patch_list.push(JsonPatch::Remove { path: path.into() });
            Ok(patch_list)
        });
        self
    }

    pub fn move_to<T: Serialize>(
        mut self,
        from: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        self.res = self.res.and_then(|mut patch_list| {
            patch_list.push(JsonPatch::Move {
                from: from.into(),
                path: path.into(),
            });
            Ok(patch_list)
        });
        self
    }

    pub fn copy_to<T: Serialize>(
        mut self,
        from: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        self.res = self.res.and_then(|mut patch_list| {
            patch_list.push(JsonPatch::Copy {
                from: from.into(),
                path: path.into(),
            });
            Ok(patch_list)
        });
        self
    }

    pub fn test<T: Serialize>(mut self, path: impl Into<String>, value: &T) -> Self {
        self.res = self.res.and_then(|mut patch_list| {
            patch_list.push(JsonPatch::Test {
                path: path.into(),
                value: serde_json::to_value(value)?,
            });
            Ok(patch_list)
        });
        self
    }
}
