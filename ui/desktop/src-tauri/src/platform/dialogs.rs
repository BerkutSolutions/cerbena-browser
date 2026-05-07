#[cfg(target_os = "windows")]
#[path = "dialogs_windows.rs"]
mod imp;
#[cfg(not(target_os = "windows"))]
#[path = "dialogs_linux.rs"]
mod imp;

pub fn pick_folder() -> Result<String, String> {
    imp::pick_folder()
}

pub fn pick_file(title: &str, filter: &str) -> Result<String, String> {
    imp::pick_file(title, filter)
}

pub fn pick_certificate_files() -> Result<Vec<String>, String> {
    imp::pick_certificate_files()
}
