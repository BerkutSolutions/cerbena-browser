use super::*;

pub(crate) fn verify_release_candidate_impl(
    candidate: &ReleaseCandidate,
    asset_bytes: &[u8],
) -> Result<(), String> {
    let asset_name = candidate
        .asset_name
        .as_deref()
        .ok_or_else(|| "release asset name is missing".to_string())?;
    let security_bundle = verify_release_security_bundle_impl(candidate)?;
    ensure_asset_matches_verified_checksum_impl(&security_bundle, asset_name, asset_bytes)
}

pub(crate) fn verify_release_security_bundle_impl(
    candidate: &ReleaseCandidate,
) -> Result<VerifiedReleaseSecurityBundle, String> {
    let checksums_url = candidate
        .checksums_url
        .as_deref()
        .ok_or_else(|| "release checksums asset is missing".to_string())?;
    let signature_url = candidate
        .checksums_signature_url
        .as_deref()
        .ok_or_else(|| "release checksums signature asset is missing".to_string())?;

    let client = build_release_http_client(Duration::from_secs(30), false)
        .map_err(|e| format!("build checksum verification client: {e}"))?;
    let checksums_bytes = download_release_bytes(&client, checksums_url, "release checksums")?;
    let signature_bytes =
        download_release_bytes(&client, signature_url, "release checksums signature")?;
    verify_release_checksums_signature_impl(&checksums_bytes, &signature_bytes)?;
    let checksums_text = String::from_utf8(checksums_bytes)
        .map_err(|e| format!("decode release checksums as utf8: {e}"))?;
    Ok(VerifiedReleaseSecurityBundle { checksums_text })
}

pub(crate) fn ensure_asset_matches_verified_checksum_impl(
    security_bundle: &VerifiedReleaseSecurityBundle,
    asset_name: &str,
    asset_bytes: &[u8],
) -> Result<(), String> {
    let expected_sha256 = extract_checksum_for_asset_impl(&security_bundle.checksums_text, asset_name)
        .ok_or_else(|| format!("signed checksums do not include {asset_name}"))?;
    let actual_sha256 = sha256_hex(asset_bytes);
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256.trim()) {
        return Err(format!(
            "update asset checksum mismatch for {asset_name}: expected {}, got {}",
            expected_sha256.trim(),
            actual_sha256
        ));
    }
    Ok(())
}

pub(crate) fn verify_release_checksums_signature_impl(
    checksums_bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<(), String> {
    let signature_b64 = String::from_utf8(signature_bytes.to_vec())
        .map_err(|e| format!("decode release signature as utf8: {e}"))?;
    let raw_signature = signature_b64.trim();
    let variants = signature_verification_variants_impl(checksums_bytes);
    let mut last_error = String::new();
    for variant in variants {
        match verify_release_checksums_signature_variant_impl(&variant, raw_signature) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

pub(crate) fn release_signing_public_keys_impl() -> Vec<String> {
    let mut keys = Vec::new();
    let current = RELEASE_SIGNING_PUBLIC_KEY_XML.trim();
    if !current.is_empty() {
        keys.push(current.to_string());
    }

    if let Ok(legacy_keys) =
        serde_json::from_str::<Vec<String>>(RELEASE_SIGNING_LEGACY_PUBLIC_KEYS_JSON)
    {
        for key in legacy_keys {
            let trimmed = key.trim();
            if !trimmed.is_empty() && keys.iter().all(|existing| existing != trimmed) {
                keys.push(trimmed.to_string());
            }
        }
    }

    keys
}

pub(crate) fn verify_release_checksums_signature_variant_impl(
    checksums_bytes: &[u8],
    signature_b64: &str,
) -> Result<(), String> {
    let mut last_error = String::new();
    for public_key_xml in release_signing_public_keys_impl() {
        match verify_release_checksums_signature_variant_with_key_impl(
            checksums_bytes,
            signature_b64,
            &public_key_xml,
        ) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

pub(crate) fn verify_release_checksums_signature_variant_with_key_impl(
    checksums_bytes: &[u8],
    signature_b64: &str,
    public_key_xml: &str,
) -> Result<(), String> {
    release_security::verify_release_checksums_signature(
        checksums_bytes,
        signature_b64,
        public_key_xml,
        RELEASE_CHECKSUMS_B64_ENV,
        RELEASE_CHECKSUMS_SIGNATURE_B64_ENV,
    )
}

pub(crate) fn signature_verification_variants_impl(checksums_bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut variants = vec![checksums_bytes.to_vec()];
    let Ok(text) = String::from_utf8(checksums_bytes.to_vec()) else {
        return variants;
    };

    let normalized_lf = text.replace("\r\n", "\n").replace('\r', "\n");
    for candidate in [normalized_lf.clone(), normalized_lf.replace('\n', "\r\n")] {
        let candidate_bytes = candidate.into_bytes();
        if variants.iter().all(|existing| existing != &candidate_bytes) {
            variants.push(candidate_bytes);
        }
    }
    variants
}

pub(crate) fn extract_checksum_for_asset_impl<'a>(
    checksums_text: &'a str,
    asset_name: &str,
) -> Option<&'a str> {
    for line in checksums_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let hash = parts.next()?;
        let entry = parts.next()?;
        let normalized = entry.replace('\\', "/");
        if normalized == asset_name || normalized.ends_with(&format!("/{asset_name}")) {
            return Some(hash);
        }
    }
    None
}
