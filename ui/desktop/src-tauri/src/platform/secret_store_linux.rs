use std::path::Path;

use sha2::{Digest, Sha256};

pub fn derive_app_secret_material(
    app_data_dir: &Path,
    current_exe: &Path,
    identifier: &str,
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(identifier.trim().as_bytes());
    hasher.update(b"|");
    hasher.update(app_data_dir.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(current_exe.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(std::env::var("USERNAME").unwrap_or_default().trim().as_bytes());
    hasher.update(b"|");
    hasher.update(std::env::var("USERDOMAIN").unwrap_or_default().trim().as_bytes());
    hasher.update(b"|");
    hasher.update(std::env::var("COMPUTERNAME").unwrap_or_default().trim().as_bytes());
    hasher.update(b"|");
    hasher.update(
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default()
            .trim()
            .as_bytes(),
    );
    Ok(hex_string(&hasher.finalize()))
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
