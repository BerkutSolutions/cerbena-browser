use browser_profile::{
    validate_modal_payload, AuditFilter, CreateProfileInput, Engine, PatchProfileInput,
    ProfileManager, ProfileModalPayload, ProfileState, SelectiveWipeRequest, WipeDataType,
};
use rusqlite::{params, Connection};
use tempfile::tempdir;

#[path = "profile_manager_tests_core/helpers.rs"]
mod helpers;
use helpers::*;

#[path = "profile_manager_tests_core/lifecycle.rs"]
mod lifecycle;
#[path = "profile_manager_tests_core/wipe.rs"]
mod wipe;
#[path = "profile_manager_tests_core/runtime_security.rs"]
mod runtime_security;
