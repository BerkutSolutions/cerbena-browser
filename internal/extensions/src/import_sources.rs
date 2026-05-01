use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSourceKind {
    LocalFolder,
    LocalArchive,
    Url,
}

#[derive(Debug, Clone)]
pub struct ImportSource {
    pub kind: ImportSourceKind,
    pub value: String,
}

#[derive(Debug, Error)]
pub enum SourceValidationError {
    #[error("empty import source")]
    Empty,
    #[error("unsupported archive extension")]
    UnsupportedArchive,
    #[error("unsupported source url")]
    UnsupportedUrl,
}

#[derive(Debug, Default, Clone)]
pub struct SourceValidator;

impl SourceValidator {
    pub fn validate(&self, source: &ImportSource) -> Result<(), SourceValidationError> {
        if source.value.trim().is_empty() {
            return Err(SourceValidationError::Empty);
        }

        match source.kind {
            ImportSourceKind::LocalFolder => Ok(()),
            ImportSourceKind::LocalArchive => validate_archive(&source.value),
            ImportSourceKind::Url => validate_url(&source.value),
        }
    }
}

fn validate_archive(path: &str) -> Result<(), SourceValidationError> {
    let lower = path.to_lowercase();
    if lower.ends_with(".zip") || lower.ends_with(".crx") || lower.ends_with(".xpi") {
        return Ok(());
    }
    Err(SourceValidationError::UnsupportedArchive)
}

fn validate_url(url: &str) -> Result<(), SourceValidationError> {
    let lower = url.to_lowercase();
    let https = lower.starts_with("https://");
    let supported = lower.contains("chromewebstore.google.com")
        || lower.contains("addons.mozilla.org")
        || lower.contains("github.com");
    if https && supported {
        return Ok(());
    }
    Err(SourceValidationError::UnsupportedUrl)
}
