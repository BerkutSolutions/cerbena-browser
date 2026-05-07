use std::path::Path;

#[cfg(target_os = "windows")]
#[path = "certificates_windows.rs"]
mod imp;
#[cfg(not(target_os = "windows"))]
#[path = "certificates_linux.rs"]
mod imp;

pub fn load_certificate_metadata(path: &Path) -> Result<(Option<String>, Option<String>), String> {
    imp::load_certificate_metadata(path)
}
