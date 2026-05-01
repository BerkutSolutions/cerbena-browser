pub mod default_browser;
pub mod home;
pub mod hooks;
pub mod local_api;
pub mod panic;
pub mod pip;
pub mod search;
pub mod security;

pub use default_browser::{DefaultBrowserHandler, LinkDispatchResult};
pub use home::{HomeAction, HomeDashboardModel, HomeMetric, HomePageService};
pub use hooks::{HookExecutionResult, LaunchHookPolicy, LaunchHookService};
pub use local_api::{ApiSession, LocalApi, LocalApiError, ProfileScopeGrant, RequestContext};
pub use panic::{PanicMode, PanicWipeService, PanicWipeSummary};
pub use pip::{PipMode, PipPolicyService, PipSetting};
pub use search::{SearchProvider, SearchProviderRegistry};
pub use security::{ApiRole, ConsentGrant, GuardrailError, SecurityGuardrails};
