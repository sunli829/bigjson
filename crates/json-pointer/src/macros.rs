#[macro_export]
macro_rules! json_pointer {
    ($path:expr) => {
        <$crate::JsonPointer as ::std::str::FromStr>::from_str($path).expect("valid json pointer")
    };
}
