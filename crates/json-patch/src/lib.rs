use json_pointer::JsonPointer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum JsonPatch {
    Add {
        path: JsonPointer,
        value: Value,
    },
    Remove {
        path: JsonPointer,
    },
    Replace {
        path: JsonPointer,
        value: Value,
    },
    Move {
        from: JsonPointer,
        path: JsonPointer,
    },
    Copy {
        from: JsonPointer,
        path: JsonPointer,
    },
}
