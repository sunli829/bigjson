use json_patch::JsonPatch;
use json_pointer::{JsonPointer, JsonPointerRef, ValueExt};
use memdb::MemDb;
use serde_json::Value;

use crate::state::SubscriptionHashMap;

enum TargetPath<'a> {
    Parent(JsonPointerRef<'a>),
    Child(JsonPointerRef<'a>),
    OtherBranch,
}

pub(crate) fn publish(
    mdb: &MemDb,
    subscriptions: &SubscriptionHashMap,
    prefix: Option<&JsonPointer>,
    patch: &[JsonPatch],
) {
    for (path, sender) in subscriptions {
        let subscription_patch = create_subscription_patch(mdb, path.as_ref(), prefix, patch);
        if !subscription_patch.is_empty() {
            let _ = sender.send(subscription_patch.into());
        }
    }
}

fn diff_path<'a>(
    subscription_path: JsonPointerRef<'a>,
    target_path: JsonPointerRef<'a>,
) -> TargetPath<'a> {
    if let Some(rel_path) = target_path.strip_prefix(subscription_path) {
        TargetPath::Child(rel_path)
    } else if let Some(rel_path) = subscription_path.strip_prefix(target_path) {
        TargetPath::Parent(rel_path)
    } else {
        TargetPath::OtherBranch
    }
}

fn create_patch_add(
    subscription_path: JsonPointerRef<'_>,
    path: JsonPointerRef<'_>,
    value: &Value,
    output: &mut Vec<JsonPatch>,
) {
    match diff_path(subscription_path, path) {
        TargetPath::Parent(rel_path) => {
            if let Some(value) = value.locate(rel_path) {
                output.push(JsonPatch::Add {
                    path: JsonPointer::root(),
                    value: value.clone(),
                });
            }
        }
        TargetPath::Child(rel_path) => {
            output.push(JsonPatch::Add {
                path: rel_path.to_owned(),
                value: value.clone(),
            });
        }
        TargetPath::OtherBranch => {}
    }
}

fn create_patch_remove(
    subscription_path: JsonPointerRef<'_>,
    path: JsonPointerRef<'_>,
    output: &mut Vec<JsonPatch>,
) {
    match diff_path(subscription_path, path) {
        TargetPath::Parent(_) => {
            output.push(JsonPatch::Add {
                path: JsonPointer::root(),
                value: Value::Null,
            });
        }
        TargetPath::Child(rel_path) => {
            if !rel_path.is_empty() {
                output.push(JsonPatch::Remove {
                    path: rel_path.to_owned(),
                });
            } else {
                output.push(JsonPatch::Add {
                    path: JsonPointer::root(),
                    value: Value::Null,
                });
            }
        }
        TargetPath::OtherBranch => {}
    }
}

fn create_patch_replace(
    subscription_path: JsonPointerRef<'_>,
    path: JsonPointerRef<'_>,
    value: &Value,
    output: &mut Vec<JsonPatch>,
) {
    match diff_path(subscription_path, path) {
        TargetPath::Parent(rel_path) => {
            if let Some(value) = value.locate(rel_path) {
                output.push(JsonPatch::Replace {
                    path: JsonPointer::root(),
                    value: value.clone(),
                });
            }
        }
        TargetPath::Child(rel_path) => {
            output.push(JsonPatch::Replace {
                path: rel_path.to_owned(),
                value: value.clone(),
            });
        }
        TargetPath::OtherBranch => {}
    }
}

fn create_patch_move(
    mdb: &MemDb,
    subscription_path: JsonPointerRef<'_>,
    from: JsonPointerRef<'_>,
    path: JsonPointerRef<'_>,
    output: &mut Vec<JsonPatch>,
) {
    match (
        diff_path(subscription_path, from),
        diff_path(subscription_path, path),
    ) {
        (TargetPath::Child(rel_path_from), TargetPath::Child(rel_path_to)) => {
            output.push(JsonPatch::Move {
                from: rel_path_from.to_owned(),
                path: rel_path_to.to_owned(),
            });
        }
        (TargetPath::OtherBranch, TargetPath::Child(rel_path_to)) => {
            if let Some(value) = mdb.get(path) {
                output.push(JsonPatch::Add {
                    path: rel_path_to.to_owned(),
                    value: value.clone(),
                });
            }
        }
        (TargetPath::Child(_), TargetPath::Parent(_))
        | (TargetPath::Parent(_), TargetPath::Parent(_))
        | (TargetPath::OtherBranch, TargetPath::Parent(_)) => {
            output.push(JsonPatch::Add {
                path: JsonPointer::root(),
                value: mdb.get(subscription_path).cloned().unwrap_or(Value::Null),
            });
        }
        (TargetPath::Parent(_), TargetPath::OtherBranch) => {
            output.push(JsonPatch::Add {
                path: JsonPointer::root(),
                value: Value::Null,
            });
        }
        (TargetPath::Child(rel_path), TargetPath::OtherBranch) => {
            if !rel_path.is_empty() {
                output.push(JsonPatch::Remove {
                    path: rel_path.to_owned(),
                });
            } else {
                output.push(JsonPatch::Add {
                    path: JsonPointer::root(),
                    value: Value::Null,
                });
            }
        }
        (TargetPath::OtherBranch, TargetPath::OtherBranch) => {}
        (TargetPath::Parent(_), TargetPath::Child(_)) => unreachable!(),
    }
}

fn create_patch_copy(
    mdb: &MemDb,
    subscription_path: JsonPointerRef<'_>,
    from: JsonPointerRef<'_>,
    path: JsonPointerRef<'_>,
    output: &mut Vec<JsonPatch>,
) {
    match (
        diff_path(subscription_path, from),
        diff_path(subscription_path, path),
    ) {
        (TargetPath::Child(rel_path_from), TargetPath::Child(rel_path_to)) => {
            output.push(JsonPatch::Copy {
                from: rel_path_from.to_owned(),
                path: rel_path_to.to_owned(),
            });
        }
        (TargetPath::Child(_), TargetPath::Parent(_))
        | (TargetPath::Parent(_), TargetPath::Parent(_))
        | (TargetPath::OtherBranch, TargetPath::Parent(_)) => {
            output.push(JsonPatch::Add {
                path: JsonPointer::root(),
                value: mdb.get(subscription_path).cloned().unwrap_or(Value::Null),
            });
        }
        (TargetPath::OtherBranch, TargetPath::Child(rel_path_to))
        | (TargetPath::Parent(_), TargetPath::Child(rel_path_to)) => {
            output.push(JsonPatch::Add {
                path: rel_path_to.to_owned(),
                value: mdb.get(path).cloned().unwrap_or(Value::Null),
            });
        }
        (TargetPath::Parent(_), TargetPath::OtherBranch)
        | (TargetPath::Child(_), TargetPath::OtherBranch)
        | (TargetPath::OtherBranch, TargetPath::OtherBranch) => {}
    }
}

fn create_subscription_patch(
    mdb: &MemDb,
    subscription_path: JsonPointerRef<'_>,
    prefix: Option<&JsonPointer>,
    patch_list: &[JsonPatch],
) -> Vec<JsonPatch> {
    let mut new_patch_list = Vec::new();

    for patch in patch_list {
        match patch {
            JsonPatch::Add { path, value } => create_patch_add(
                subscription_path,
                path.with_prefix_opt(prefix),
                value,
                &mut new_patch_list,
            ),
            JsonPatch::Remove { path } => create_patch_remove(
                subscription_path,
                path.with_prefix_opt(prefix),
                &mut new_patch_list,
            ),
            JsonPatch::Replace { path, value } => create_patch_replace(
                subscription_path,
                path.with_prefix_opt(prefix),
                value,
                &mut new_patch_list,
            ),
            JsonPatch::Move { from, path } => create_patch_move(
                mdb,
                subscription_path,
                from.with_prefix_opt(prefix),
                path.with_prefix_opt(prefix),
                &mut new_patch_list,
            ),
            JsonPatch::Copy { from, path } => create_patch_copy(
                mdb,
                subscription_path,
                from.with_prefix_opt(prefix),
                path.with_prefix_opt(prefix),
                &mut new_patch_list,
            ),
        }
    }

    new_patch_list
}

#[cfg(test)]
mod tests {
    use json_pointer::json_pointer;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_add() {
        // Add to root
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Add {
                path: json_pointer!(""),
                value: json!({
                    "a": {
                        "b": {
                            "c": {
                                "d": 10,
                            },
                            "e": 20,
                        }
                    }
                }),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!(""),
                value: json!({ "d": 10 }),
            }],
            "Add to root"
        );

        // Add to parent
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Add {
                path: json_pointer!("/a/b"),
                value: json!({
                    "c": {
                        "d": 10,
                    },
                    "e": 20,
                }),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: JsonPointer::root(),
                value: json!({
                    "d": 10
                }),
            }],
            "Add to parent"
        );

        // Add to subscription root
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Add {
                path: json_pointer!("/a/b/c"),
                value: json!({
                    "e": 20,
                }),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: JsonPointer::root(),
                value: json!({
                    "e": 20
                }),
            }],
            "Add to subscription root"
        );

        // Add to child
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Add {
                path: json_pointer!("/a/b/c/d/1"),
                value: json!({
                    "e": 10,
                    "f": [1, 2, 3]
                }),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!("/d/1"),
                value: json!({
                    "e": 10,
                    "f": [1, 2, 3]
                }),
            }],
            "Add to child"
        );

        // Add to other branch
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/k/j").as_ref(),
            None,
            &[JsonPatch::Add {
                path: json_pointer!("/a/b/c/d/1"),
                value: json!({
                    "e": 10,
                    "f": [1, 2, 3]
                }),
            }],
        );
        assert_eq!(patch, vec![], "Add to other branch");
    }

    #[test]
    fn test_remove() {
        // Remove parent
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Remove {
                path: json_pointer!("/a/b"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!(""),
                value: json!(null),
            }],
            "Remove parent"
        );

        // Remove subscription root
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Remove {
                path: json_pointer!("/a/b/c"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!(""),
                value: json!(null),
            }],
            "Remove subscription root"
        );

        // remove child
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Remove {
                path: json_pointer!("/a/b/c/d/1"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Remove {
                path: json_pointer!("/d/1"),
            }],
            "remove child"
        );

        // Remove other branch
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Remove {
                path: json_pointer!("/k/a"),
            }],
        );
        assert_eq!(patch, vec![], "Remove other branch");
    }

    #[test]
    fn test_replace() {
        // Replace root
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Replace {
                path: json_pointer!(""),
                value: json!({
                    "a": {
                        "b": {
                            "c": {
                                "d": 10,
                            },
                            "e": 20,
                        }
                    }
                }),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Replace {
                path: json_pointer!(""),
                value: json!({ "d": 10 }),
            }],
            "Replace root"
        );

        // Replace parent
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Replace {
                path: json_pointer!("/a/b"),
                value: json!({
                    "c": {
                        "d": 10,
                    },
                    "e": 20,
                }),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Replace {
                path: json_pointer!(""),
                value: json!({
                    "d": 10
                }),
            }],
            "Replace parent"
        );

        // Replace subscription root
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Replace {
                path: json_pointer!("/a/b/c"),
                value: json!({
                    "d": 20,
                }),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Replace {
                path: json_pointer!(""),
                value: json!({
                    "d": 20
                }),
            }],
            "Replace subscription root"
        );

        // Replace child
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Replace {
                path: json_pointer!("/a/b/c/d/1"),
                value: json!({
                    "e": 10,
                    "f": [1, 2, 3]
                }),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Replace {
                path: json_pointer!("/d/1"),
                value: json!({
                    "e": 10,
                    "f": [1, 2, 3]
                }),
            }],
            "Replace child"
        );

        // Replace other branch
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Replace {
                path: json_pointer!("/k/j"),
                value: json!(10),
            }],
        );
        assert_eq!(patch, vec![], "Replace other branch");
    }

    #[test]
    fn test_move() {
        // Move from child to child
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Move {
                from: json_pointer!("/a/b/c/d/1/e"),
                path: json_pointer!("/a/b/c/k/2"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Move {
                from: json_pointer!("/d/1/e"),
                path: json_pointer!("/k/2"),
            }],
            "Move from child to child"
        );

        // Move from parent to child(impossible)

        // Move from other branch to child
        let patch = create_subscription_patch(
            &MemDb::new(json!({
                "a": {
                    "b": {
                        "c": {
                            "d": 100
                        }
                    }
                }
            })),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Move {
                from: json_pointer!("/k/j"),
                path: json_pointer!("/a/b/c/d"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!("/d"),
                value: json!(100),
            }],
            "Move from other branch to child"
        );

        // Move from child to parent
        let patch = create_subscription_patch(
            &MemDb::new(json!({
                "a": {
                    "b": {
                        "c": {
                            "d": 10
                        }
                    }
                }
            })),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Move {
                from: json_pointer!("/a/b/c/d/a"),
                path: json_pointer!("/a"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!(""),
                value: json!({
                    "d": 10
                }),
            }],
            "Move from child to parent"
        );

        // Move from child to parent 2
        let patch = create_subscription_patch(
            &MemDb::new(json!({
                "k": {
                    "j": 10
                }
            })),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Move {
                from: json_pointer!("/a/b/c/d/a"),
                path: json_pointer!("/a"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!(""),
                value: Value::Null,
            }],
            "Move from child to parent 2"
        );

        // Move from child to subscription root
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Move {
                from: json_pointer!("/a/b/c/d/a/b/c"),
                path: json_pointer!("/a/b/c"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Move {
                from: json_pointer!("/d/a/b/c"),
                path: json_pointer!(""),
            }],
            "Move from child to subscription root"
        );

        // Move from parent to other branch
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Move {
                from: json_pointer!("/a/b"),
                path: json_pointer!("/k"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!(""),
                value: Value::Null
            }],
            "Move from parent to other branch"
        );

        // Move from child to other branch
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Move {
                from: json_pointer!("/a/b/c/d"),
                path: json_pointer!("/k"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Remove {
                path: json_pointer!("/d"),
            }],
            "Move from child to other branch"
        );

        // Move from subscription root to other branch
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Move {
                from: json_pointer!("/a/b/c"),
                path: json_pointer!("/k"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!(""),
                value: Value::Null
            }],
            "Move from subscription root to other branch"
        );

        // Move from other branch root to other branch
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Move {
                from: json_pointer!("/k"),
                path: json_pointer!("/j"),
            }],
        );
        assert_eq!(patch, vec![], "Move from other branch root to other branch");
    }

    #[test]
    fn test_copy() {
        // Copy from child to child
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Copy {
                from: json_pointer!("/a/b/c/d/1/e"),
                path: json_pointer!("/a/b/c/k/2"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Copy {
                from: json_pointer!("/d/1/e"),
                path: json_pointer!("/k/2"),
            }],
            "Copy from child to child"
        );

        // Copy from parent to child
        let patch = create_subscription_patch(
            &MemDb::new(json!({
                "a": {
                    "b": {
                        "c": {
                            "d": 10
                        }
                    }
                }
            })),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Copy {
                from: json_pointer!("/a/b"),
                path: json_pointer!("/a/b/c/d"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!("/d"),
                value: json!(10),
            }],
            "Copy from parent to child"
        );

        // Copy from parent to subscription root
        let patch = create_subscription_patch(
            &MemDb::new(json!({
                "a": {
                    "b": {
                        "c": {
                            "d": 10
                        }
                    }
                }
            })),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Copy {
                from: json_pointer!("/a/b"),
                path: json_pointer!("/a/b/c"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!(""),
                value: json!({
                    "d": 10
                }),
            }],
            "Copy from parent to subscription root"
        );

        // Copy from child to parent
        let patch = create_subscription_patch(
            &MemDb::new(json!({
                "a": {
                    "b": {
                        "c": {
                            "d": 10
                        }
                    }
                }
            })),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Copy {
                from: json_pointer!("/a/b/c/d/1/e"),
                path: json_pointer!("/a"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!(""),
                value: json!({
                    "d": 10,
                }),
            }],
            "Copy from child to parent"
        );

        // Copy from subscription root to parent
        let patch = create_subscription_patch(
            &MemDb::new(json!({
                "a": {
                    "b": {
                        "c": {
                            "d": 10
                        }
                    }
                }
            })),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Copy {
                from: json_pointer!("/a/b/c"),
                path: json_pointer!("/a"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!(""),
                value: json!({
                    "d": 10,
                }),
            }],
            "Copy from subscription root to parent"
        );

        // Copy from other branch to parent
        let patch = create_subscription_patch(
            &MemDb::new(json!({
                "a": {
                    "b": {
                        "c": {
                            "d": 10
                        }
                    }
                }
            })),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Copy {
                from: json_pointer!("/k/j"),
                path: json_pointer!("/a"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!(""),
                value: json!({
                    "d": 10,
                }),
            }],
            "Copy from subscription root to parent"
        );

        // Copy from parent to parent
        let patch = create_subscription_patch(
            &MemDb::new(json!({
                "a": {
                    "b": {
                        "c": {
                            "d": 10
                        }
                    }
                }
            })),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Copy {
                from: json_pointer!("/a/b"),
                path: json_pointer!("/a"),
            }],
        );
        assert_eq!(
            patch,
            vec![JsonPatch::Add {
                path: json_pointer!(""),
                value: json!({
                    "d": 10,
                }),
            }],
            "Copy from parent to parent"
        );

        // Copy from child to other branch
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Copy {
                from: json_pointer!("/a/b/c/d"),
                path: json_pointer!("/k"),
            }],
        );
        assert_eq!(patch, vec![], "Copy from child to other branch");

        // Copy from parent to other branch
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Copy {
                from: json_pointer!("/a"),
                path: json_pointer!("/k"),
            }],
        );
        assert_eq!(patch, vec![], "Copy from parent to other branch");

        // Copy from subscription root to other branch
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Copy {
                from: json_pointer!("/a/b/c"),
                path: json_pointer!("/k"),
            }],
        );
        assert_eq!(patch, vec![], "Copy from subscription root to other branch");

        // Copy from other branch to other branch
        let patch = create_subscription_patch(
            &MemDb::default(),
            json_pointer!("/a/b/c").as_ref(),
            None,
            &[JsonPatch::Copy {
                from: json_pointer!("/k/j"),
                path: json_pointer!("/u"),
            }],
        );
        assert_eq!(patch, vec![], "Copy from other branch to other branch");
    }
}
