use std::path::Path;

#[cfg(target_os = "windows")]
#[path = "secret_store_windows.rs"]
mod imp;
#[cfg(not(target_os = "windows"))]
#[path = "secret_store_linux.rs"]
mod imp;

pub fn derive_app_secret_material(
    app_data_dir: &Path,
    current_exe: &Path,
    identifier: &str,
) -> Result<String, String> {
    imp::derive_app_secret_material(app_data_dir, current_exe, identifier)
}
