pub fn pick_folder() -> Result<String, String> {
    Err("folder picker is not supported on Linux yet".to_string())
}

pub fn pick_file(_title: &str, _filter: &str) -> Result<String, String> {
    Err("file picker is not supported on Linux yet".to_string())
}

pub fn pick_certificate_files() -> Result<Vec<String>, String> {
    Err("certificate picker is not supported on Linux yet".to_string())
}
