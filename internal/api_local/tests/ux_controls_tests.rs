use browser_api_local::{
    LaunchHookPolicy, LaunchHookService, PipMode, PipPolicyService, SearchProvider,
    SearchProviderRegistry,
};

#[test]
fn pip_policy_falls_back_when_platform_unsupported() {
    let service = PipPolicyService;
    let setting = service.resolve(PipMode::Enabled, false);
    assert_eq!(setting.mode, PipMode::Disabled);
}

#[test]
fn launch_hook_validation_and_timeout_behavior() {
    let hooks = LaunchHookService;
    let policy = LaunchHookPolicy {
        url: "https://hook.local/start".to_string(),
        timeout_ms: 1000,
        allow_insecure_http: false,
    };
    assert!(hooks.validate(&policy).is_ok());
    let timed_out = hooks.execute(&policy, 2000);
    assert!(!timed_out.executed);
}

#[test]
fn search_provider_registry_requires_query_token() {
    let mut registry = SearchProviderRegistry::default();
    let bad = registry.import_presets(vec![SearchProvider {
        id: "bad".to_string(),
        display_name: "Bad".to_string(),
        query_template: "https://search.example/".to_string(),
    }]);
    assert!(bad.is_err());
}
