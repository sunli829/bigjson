use json_patch::JsonPatch;
use json_pointer::{JsonPointer, JsonPointerRef, ToJsonPointerRef, ValueExt};
use serde_json::Value;

use crate::{
    undo_command::{UndoCommand, UpdateSource, UpdateTarget},
    MemDbError,
};

#[derive(Debug)]
pub struct MemDb {
    root: Value,
}

impl Default for MemDb {
    fn default() -> Self {
        Self {
            root: Value::Object(Default::default()),
        }
    }
}

impl MemDb {
    pub fn new(root: Value) -> Self {
        Self { root }
    }

    pub fn get(&self, path: impl ToJsonPointerRef) -> Option<&Value> {
        self.root.locate(path)
    }

    #[inline]
    pub fn root(&self) -> &Value {
        &self.root
    }

    pub fn patch(
        &mut self,
        prefix: Option<&JsonPointer>,
        commands: Vec<JsonPatch>,
    ) -> Result<(), MemDbError> {
        let mut commands = commands;
        let mut undo_commands = Vec::new();

        match self.patch_all(&mut undo_commands, prefix, &mut commands) {
            Ok(()) => Ok(()),
            Err(err) => {
                for undo_command in undo_commands.into_iter().rev() {
                    undo_command.execute(&mut self.root);
                }
                Err(err)
            }
        }
    }

    fn patch_all<'a>(
        &mut self,
        undo_commands: &mut Vec<UndoCommand<'a>>,
        prefix: Option<&'a JsonPointer>,
        commands: &'a mut [JsonPatch],
    ) -> Result<(), MemDbError> {
        for command in commands {
            self.patch_command(undo_commands, prefix, command)?;
        }
        Ok(())
    }

    fn patch_command<'a>(
        &mut self,
        undo_commands: &mut Vec<UndoCommand<'a>>,
        prefix: Option<&'a JsonPointer>,
        command: &'a mut JsonPatch,
    ) -> Result<(), MemDbError> {
        match command {
            JsonPatch::Add { path, value } => self.patch_command_add(
                undo_commands,
                path.with_prefix_opt(prefix),
                std::mem::take(value),
            ),
            JsonPatch::Remove { path } => {
                self.patch_command_remove(undo_commands, path.with_prefix_opt(prefix))
            }
            JsonPatch::Replace { path, value } => self.patch_command_replace(
                undo_commands,
                path.with_prefix_opt(prefix),
                std::mem::take(value),
            ),
            JsonPatch::Move { from, path } => self.patch_command_move(
                undo_commands,
                from.with_prefix_opt(prefix),
                path.with_prefix_opt(prefix),
            ),
            JsonPatch::Copy { from, path } => self.patch_command_copy(
                undo_commands,
                from.with_prefix_opt(prefix),
                path.with_prefix_opt(prefix),
            ),
            JsonPatch::Test { path, value } => {
                self.patch_command_test(path.with_prefix_opt(prefix), value)
            }
        }
    }

    fn patch_command_add<'a>(
        &mut self,
        undo_commands: &mut Vec<UndoCommand<'a>>,
        path: JsonPointerRef<'a>,
        value: Value,
    ) -> Result<(), MemDbError> {
        match path.split_last() {
            Some((parent_path, key)) => {
                let parent =
                    self.root
                        .locate_mut(parent_path)
                        .ok_or_else(|| MemDbError::PathNotFound {
                            path: parent_path.to_owned(),
                        })?;
                match parent {
                    Value::Object(obj) => {
                        let prev_value = obj.insert(key.to_string(), value);
                        undo_commands.push(UndoCommand::Add {
                            target: UpdateTarget::Object {
                                path: parent_path,
                                key,
                            },
                            prev_value,
                        });
                    }
                    Value::Array(array) => {
                        if key == "-" {
                            array.push(value);
                            undo_commands.push(UndoCommand::Add {
                                target: UpdateTarget::ArrayAppend { path: parent_path },
                                prev_value: None,
                            });
                        } else {
                            let index = key
                                .parse::<usize>()
                                .ok()
                                .filter(|index| *index <= array.len())
                                .ok_or_else(|| MemDbError::InvalidIndex {
                                    path: parent_path.to_owned(),
                                    index: key.to_string(),
                                })?;
                            array.insert(index, value);
                            undo_commands.push(UndoCommand::Add {
                                target: UpdateTarget::ArrayInsert {
                                    path: parent_path,
                                    index,
                                },
                                prev_value: None,
                            });
                        }
                    }
                    _ => {
                        return Err(MemDbError::NotAContainer {
                            path: parent_path.to_owned(),
                        })
                    }
                }
            }
            None => {
                let prev_value = std::mem::replace(&mut self.root, value);
                undo_commands.push(UndoCommand::ReplaceRoot { prev_value });
            }
        }

        Ok(())
    }

    fn patch_command_remove<'a>(
        &mut self,
        undo_commands: &mut Vec<UndoCommand<'a>>,
        path: JsonPointerRef<'a>,
    ) -> Result<(), MemDbError> {
        let (parent_path, key) = path.split_last().ok_or(MemDbError::EmptyPath)?;
        let parent = self
            .root
            .locate_mut(parent_path)
            .ok_or_else(|| MemDbError::PathNotFound {
                path: parent_path.to_owned(),
            })?;

        match parent {
            Value::Object(obj) => {
                let prev_value = obj.remove(key).ok_or_else(|| MemDbError::PathNotFound {
                    path: path.to_owned(),
                })?;
                undo_commands.push(UndoCommand::Remove {
                    source: UpdateSource::Object {
                        path: parent_path,
                        key,
                    },
                    prev_value,
                });
            }
            Value::Array(array) => {
                let index = key
                    .parse::<usize>()
                    .ok()
                    .filter(|index| *index < array.len())
                    .ok_or_else(|| MemDbError::InvalidIndex {
                        path: parent_path.to_owned(),
                        index: key.to_string(),
                    })?;
                let prev_value = array.remove(index);
                undo_commands.push(UndoCommand::Remove {
                    source: UpdateSource::Array {
                        path: parent_path,
                        index,
                    },
                    prev_value,
                });
            }
            _ => {
                return Err(MemDbError::NotAContainer {
                    path: parent_path.to_owned(),
                })
            }
        }

        Ok(())
    }

    fn patch_command_replace<'a>(
        &mut self,
        undo_commands: &mut Vec<UndoCommand<'a>>,
        path: JsonPointerRef<'a>,
        value: Value,
    ) -> Result<(), MemDbError> {
        let prev_value = self
            .root
            .locate_mut(path)
            .ok_or_else(|| MemDbError::PathNotFound {
                path: path.to_owned(),
            })?;
        undo_commands.push(UndoCommand::Replace {
            path,
            prev_value: std::mem::replace(prev_value, value),
        });
        Ok(())
    }

    fn patch_command_move<'a>(
        &mut self,
        undo_commands: &mut Vec<UndoCommand<'a>>,
        from: JsonPointerRef<'a>,
        path: JsonPointerRef<'a>,
    ) -> Result<(), MemDbError> {
        let (parent_path, key) = from.split_last().ok_or(MemDbError::EmptyPath)?;

        let (source, value) = {
            let parent =
                self.root
                    .locate_mut(parent_path)
                    .ok_or_else(|| MemDbError::PathNotFound {
                        path: parent_path.to_owned(),
                    })?;
            match parent {
                Value::Object(obj) => {
                    let value = obj.remove(key).ok_or_else(|| MemDbError::PathNotFound {
                        path: path.to_owned(),
                    })?;
                    (
                        UpdateSource::Object {
                            path: parent_path,
                            key,
                        },
                        value,
                    )
                }
                Value::Array(array) => {
                    let index = key
                        .parse::<usize>()
                        .ok()
                        .filter(|index| *index < array.len())
                        .ok_or_else(|| MemDbError::InvalidIndex {
                            path: parent_path.to_owned(),
                            index: key.to_string(),
                        })?;
                    let value = array.remove(index);
                    (
                        UpdateSource::Array {
                            path: parent_path,
                            index,
                        },
                        value,
                    )
                }
                _ => {
                    return Err(MemDbError::NotAContainer {
                        path: parent_path.to_owned(),
                    })
                }
            }
        };

        match path.split_last() {
            Some((parent_path, key)) => {
                let parent =
                    self.root
                        .locate_mut(parent_path)
                        .ok_or_else(|| MemDbError::PathNotFound {
                            path: parent_path.to_owned(),
                        })?;
                let (target, prev_value) = match parent {
                    Value::Object(obj) => {
                        let prev_value = obj.insert(key.to_string(), value);
                        (
                            UpdateTarget::Object {
                                path: parent_path,
                                key,
                            },
                            prev_value,
                        )
                    }
                    Value::Array(array) => {
                        if key == "-" {
                            array.push(value);
                            (UpdateTarget::ArrayAppend { path: parent_path }, None)
                        } else {
                            let index = key
                                .parse::<usize>()
                                .ok()
                                .filter(|index| *index <= array.len())
                                .ok_or_else(|| MemDbError::InvalidIndex {
                                    path: parent_path.to_owned(),
                                    index: key.to_string(),
                                })?;
                            array.insert(index, value);
                            (
                                UpdateTarget::ArrayInsert {
                                    path: parent_path,
                                    index,
                                },
                                None,
                            )
                        }
                    }
                    _ => {
                        return Err(MemDbError::NotAContainer {
                            path: parent_path.to_owned(),
                        })
                    }
                };

                undo_commands.push(UndoCommand::Move {
                    source,
                    target,
                    prev_value,
                });
            }
            None => {
                let prev_value = std::mem::replace(&mut self.root, value);
                undo_commands.push(UndoCommand::MoveToRoot { source, prev_value });
            }
        }

        Ok(())
    }

    fn patch_command_copy<'a>(
        &mut self,
        undo_commands: &mut Vec<UndoCommand<'a>>,
        from: JsonPointerRef<'a>,
        path: JsonPointerRef<'a>,
    ) -> Result<(), MemDbError> {
        let (parent_path, key) = from.split_last().ok_or(MemDbError::EmptyPath)?;

        let value = {
            let parent =
                self.root
                    .locate_mut(parent_path)
                    .ok_or_else(|| MemDbError::PathNotFound {
                        path: parent_path.to_owned(),
                    })?;
            match parent {
                Value::Object(obj) => obj
                    .get(key)
                    .ok_or_else(|| MemDbError::PathNotFound {
                        path: path.to_owned(),
                    })
                    .cloned()?,
                Value::Array(array) => {
                    let index = key
                        .parse::<usize>()
                        .ok()
                        .filter(|index| *index < array.len())
                        .ok_or_else(|| MemDbError::InvalidIndex {
                            path: parent_path.to_owned(),
                            index: key.to_string(),
                        })?;
                    array[index].clone()
                }
                _ => {
                    return Err(MemDbError::NotAContainer {
                        path: parent_path.to_owned(),
                    })
                }
            }
        };

        match path.split_last() {
            Some((parent_path, key)) => {
                let parent =
                    self.root
                        .locate_mut(parent_path)
                        .ok_or_else(|| MemDbError::PathNotFound {
                            path: parent_path.to_owned(),
                        })?;
                let (target, prev_value) = match parent {
                    Value::Object(obj) => {
                        let prev_value = obj.insert(key.to_string(), value);
                        (
                            UpdateTarget::Object {
                                path: parent_path,
                                key,
                            },
                            prev_value,
                        )
                    }
                    Value::Array(array) => {
                        if key == "-" {
                            array.push(value);
                            (UpdateTarget::ArrayAppend { path: parent_path }, None)
                        } else {
                            let index = key
                                .parse::<usize>()
                                .ok()
                                .filter(|index| *index <= array.len())
                                .ok_or_else(|| MemDbError::InvalidIndex {
                                    path: parent_path.to_owned(),
                                    index: key.to_string(),
                                })?;
                            array.insert(index, value);
                            (
                                UpdateTarget::ArrayInsert {
                                    path: parent_path,
                                    index,
                                },
                                None,
                            )
                        }
                    }
                    _ => {
                        return Err(MemDbError::NotAContainer {
                            path: parent_path.to_owned(),
                        })
                    }
                };

                undo_commands.push(UndoCommand::Copy { target, prev_value });
            }
            None => {
                let prev_value = std::mem::replace(&mut self.root, value);
                undo_commands.push(UndoCommand::CopyToRoot { prev_value });
            }
        }

        Ok(())
    }

    fn patch_command_test<'a>(
        &mut self,
        path: JsonPointerRef<'a>,
        value: &Value,
    ) -> Result<(), MemDbError> {
        if self.root.locate(path).unwrap_or(&Value::Null) != value {
            Err(MemDbError::TestFailed)
        } else {
            Ok(())
        }
    }
}
