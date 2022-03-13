use serde_json::Value;

use crate::ToJsonPointerRef;

pub trait ValueExt {
    fn locate<T: ToJsonPointerRef>(&self, pointer: T) -> Option<&Value>;

    fn locate_mut<T: ToJsonPointerRef>(&mut self, pointer: T) -> Option<&mut Value>;
}

impl ValueExt for Value {
    fn locate<T: ToJsonPointerRef>(&self, pointer: T) -> Option<&Value> {
        pointer
            .to_json_pointer_ref()
            .iter()
            .try_fold(self, |acc, segment| match acc {
                Value::Object(obj) => obj.get(segment),
                Value::Array(array) => {
                    let idx = segment.parse::<usize>().ok()?;
                    array.get(idx)
                }
                _ => None,
            })
    }

    fn locate_mut<T: ToJsonPointerRef>(&mut self, pointer: T) -> Option<&mut Value> {
        pointer
            .to_json_pointer_ref()
            .iter()
            .try_fold(self, |acc, segment| match acc {
                Value::Object(obj) => obj.get_mut(segment),
                Value::Array(array) => {
                    let idx = segment.parse::<usize>().ok()?;
                    array.get_mut(idx)
                }
                _ => None,
            })
    }
}
