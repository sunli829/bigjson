use json_pointer::{JsonPointerRef, ValueExt};
use serde_json::Value;

pub enum UpdateSource<'a> {
    Object {
        path: JsonPointerRef<'a>,
        key: &'a str,
    },
    Array {
        path: JsonPointerRef<'a>,
        index: usize,
    },
}

pub enum UpdateTarget<'a> {
    Object {
        path: JsonPointerRef<'a>,
        key: &'a str,
    },
    ArrayInsert {
        path: JsonPointerRef<'a>,
        index: usize,
    },
    ArrayAppend {
        path: JsonPointerRef<'a>,
    },
}

pub(crate) enum UndoCommand<'a> {
    ReplaceRoot {
        prev_value: Value,
    },
    Add {
        target: UpdateTarget<'a>,
        prev_value: Option<Value>,
    },
    Remove {
        source: UpdateSource<'a>,
        prev_value: Value,
    },
    Replace {
        path: JsonPointerRef<'a>,
        prev_value: Value,
    },
    Move {
        source: UpdateSource<'a>,
        target: UpdateTarget<'a>,
        prev_value: Option<Value>,
    },
    MoveToRoot {
        source: UpdateSource<'a>,
        prev_value: Value,
    },
    Copy {
        target: UpdateTarget<'a>,
        prev_value: Option<Value>,
    },
    CopyToRoot {
        prev_value: Value,
    },
}

impl<'a> UndoCommand<'a> {
    pub(crate) fn execute(self, root: &mut Value) {
        match self {
            UndoCommand::ReplaceRoot { prev_value } => {
                *root = prev_value;
            }
            UndoCommand::Add {
                target: UpdateTarget::Object { path, key },
                prev_value,
            } => {
                if let Some(Value::Object(parent)) = root.locate_mut(path) {
                    match prev_value {
                        Some(prev_value) => {
                            parent.insert(key.to_string(), prev_value);
                        }
                        None => {
                            parent.remove(key);
                        }
                    }
                }
            }
            UndoCommand::Add {
                target: UpdateTarget::ArrayInsert { path, index },
                ..
            } => {
                if let Some(Value::Array(parent)) = root.locate_mut(path) {
                    parent.remove(index);
                }
            }
            UndoCommand::Add {
                target: UpdateTarget::ArrayAppend { path },
                ..
            } => {
                if let Some(Value::Array(parent)) = root.locate_mut(path) {
                    parent.pop();
                }
            }
            UndoCommand::Remove {
                source: UpdateSource::Object { path, key },
                prev_value,
            } => {
                if let Some(Value::Object(parent)) = root.locate_mut(path) {
                    parent.insert(key.to_string(), prev_value);
                }
            }
            UndoCommand::Remove {
                source: UpdateSource::Array { path, index },
                prev_value,
            } => {
                if let Some(Value::Array(parent)) = root.locate_mut(path) {
                    parent.insert(index, prev_value);
                }
            }
            UndoCommand::Replace { path, prev_value } => {
                if let Some(value) = root.locate_mut(path) {
                    *value = prev_value;
                }
            }
            UndoCommand::Move {
                source:
                    UpdateSource::Object {
                        path: from_path,
                        key: from_key,
                    },
                target:
                    UpdateTarget::Object {
                        path: to_path,
                        key: to_key,
                    },
                prev_value,
            } => {
                if let Some(Value::Object(obj)) = root.locate_mut(to_path) {
                    let value = match prev_value {
                        Some(prev_value) => obj.insert(to_key.to_string(), prev_value),
                        None => obj.remove(to_key),
                    };
                    if let Some(value) = value {
                        if let Some(Value::Object(obj)) = root.locate_mut(from_path) {
                            obj.insert(from_key.to_string(), value);
                        }
                    }
                }
            }
            UndoCommand::Move {
                source:
                    UpdateSource::Object {
                        path: from_path,
                        key: from_key,
                    },
                target:
                    UpdateTarget::ArrayInsert {
                        path: to_path,
                        index: to_index,
                    },
                ..
            } => {
                if let Some(Value::Array(array)) = root.locate_mut(to_path) {
                    let value = array.remove(to_index);
                    if let Some(Value::Object(obj)) = root.locate_mut(from_path) {
                        obj.insert(from_key.to_string(), value);
                    }
                }
            }
            UndoCommand::Move {
                source:
                    UpdateSource::Object {
                        path: from_path,
                        key: from_key,
                    },
                target: UpdateTarget::ArrayAppend { path: to_path },
                ..
            } => {
                if let Some(Value::Array(array)) = root.locate_mut(to_path) {
                    let value = array.pop();
                    if let Some(value) = value {
                        if let Some(Value::Object(obj)) = root.locate_mut(from_path) {
                            obj.insert(from_key.to_string(), value);
                        }
                    }
                }
            }
            UndoCommand::Move {
                source:
                    UpdateSource::Array {
                        path: from_path,
                        index: from_index,
                    },
                target:
                    UpdateTarget::Object {
                        path: to_path,
                        key: to_key,
                    },
                prev_value,
            } => {
                if let Some(Value::Object(obj)) = root.locate_mut(to_path) {
                    let value = match prev_value {
                        Some(prev_value) => obj.insert(to_key.to_string(), prev_value),
                        None => obj.remove(to_key),
                    };
                    if let Some(value) = value {
                        if let Some(Value::Array(array)) = root.locate_mut(from_path) {
                            array.insert(from_index, value);
                        }
                    }
                }
            }
            UndoCommand::Move {
                source:
                    UpdateSource::Array {
                        path: from_path,
                        index: from_index,
                    },
                target:
                    UpdateTarget::ArrayInsert {
                        path: to_path,
                        index: to_index,
                    },
                ..
            } => {
                if let Some(Value::Array(array)) = root.locate_mut(to_path) {
                    let value = array.remove(to_index);
                    if let Some(Value::Array(array)) = root.locate_mut(from_path) {
                        array.insert(from_index, value);
                    }
                }
            }
            UndoCommand::Move {
                source:
                    UpdateSource::Array {
                        path: from_path,
                        index: from_index,
                    },
                target: UpdateTarget::ArrayAppend { path: to_path },
                ..
            } => {
                if let Some(Value::Array(array)) = root.locate_mut(to_path) {
                    let value = array.pop();
                    if let Some(value) = value {
                        if let Some(Value::Array(array)) = root.locate_mut(from_path) {
                            array.insert(from_index, value);
                        }
                    }
                }
            }
            UndoCommand::MoveToRoot {
                source: UpdateSource::Object { path, key },
                prev_value,
            } => {
                let value = std::mem::replace(root, prev_value);
                if let Some(Value::Object(obj)) = root.locate_mut(path) {
                    obj.insert(key.to_string(), value);
                }
            }
            UndoCommand::MoveToRoot {
                source: UpdateSource::Array { path, index },
                prev_value,
            } => {
                let value = std::mem::replace(root, prev_value);
                if let Some(Value::Array(array)) = root.locate_mut(path) {
                    array.insert(index, value);
                }
            }
            UndoCommand::Copy {
                target: UpdateTarget::Object { path, key },
                prev_value,
            } => {
                if let Some(Value::Object(obj)) = root.locate_mut(path) {
                    match prev_value {
                        Some(prev_value) => obj.insert(key.to_string(), prev_value),
                        None => obj.remove(key),
                    };
                }
            }
            UndoCommand::Copy {
                target: UpdateTarget::ArrayInsert { path, index },
                ..
            } => {
                if let Some(Value::Array(array)) = root.locate_mut(path) {
                    array.remove(index);
                }
            }
            UndoCommand::Copy {
                target: UpdateTarget::ArrayAppend { path },
                ..
            } => {
                if let Some(Value::Array(array)) = root.locate_mut(path) {
                    array.pop();
                }
            }
            UndoCommand::CopyToRoot { prev_value } => {
                *root = prev_value;
            }
        }
    }
}
