#[cfg(target_os = "windows")]
#[path = "release_security_windows.rs"]
mod imp;
#[cfg(not(target_os = "windows"))]
#[path = "release_security_linux.rs"]
mod imp;

pub fn verify_release_checksums_signature(
    checksums_bytes: &[u8],
    signature_b64: &str,
    public_key_xml: &str,
    checksums_env: &str,
    signature_env: &str,
) -> Result<(), String> {
    imp::verify_release_checksums_signature(
        checksums_bytes,
        signature_b64,
        public_key_xml,
        checksums_env,
        signature_env,
    )
}
