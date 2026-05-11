use super::*;

pub(crate) fn resolved_binary_for_engine(
    engine: EngineKind,
    installation: EngineInstallation,
) -> PathBuf {
    if matches!(engine, EngineKind::Librewolf | EngineKind::FirefoxEsr) {
        prefer_librewolf_browser_binary(&installation.binary_path)
    } else {
        installation.binary_path
    }
}

pub(crate) fn dispatch_launch(
    runtime: &EngineRuntime,
    engine: EngineKind,
    request: crate::contract::LaunchRequest,
) -> Result<u32, EngineError> {
    match engine {
        EngineKind::Chromium => runtime.chromium_adapter().launch(request),
        EngineKind::UngoogledChromium => runtime.ungoogled_chromium_adapter().launch(request),
        EngineKind::FirefoxEsr => runtime.firefox_esr_adapter().launch(request),
        EngineKind::Librewolf => runtime.librewolf_adapter().launch(request),
    }
}
