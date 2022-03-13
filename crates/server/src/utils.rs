pub(crate) fn normalize_path(path: &str) -> String {
    if !path.is_empty() {
        format!("/{}", path)
    } else {
        path.to_string()
    }
}
