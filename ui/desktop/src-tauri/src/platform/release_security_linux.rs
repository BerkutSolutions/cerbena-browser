pub fn verify_release_checksums_signature(
    _checksums_bytes: &[u8],
    _signature_b64: &str,
    _public_key_xml: &str,
    _checksums_env: &str,
    _signature_env: &str,
) -> Result<(), String> {
    Err("release checksum signature verification is not supported on Linux yet".to_string())
}
