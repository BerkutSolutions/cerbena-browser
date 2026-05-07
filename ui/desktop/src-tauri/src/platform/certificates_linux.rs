use std::path::Path;

pub fn load_certificate_metadata(path: &Path) -> Result<(Option<String>, Option<String>), String> {
    let _ = path;
    Ok((None, None))
}
