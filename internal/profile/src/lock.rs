use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::errors::ProfileError;

const LOCK_SALT_LEN: usize = 16;
const LOCK_HASH_LEN: usize = 32;
const LOCK_ROUNDS: u32 = 120_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockPolicy {
    pub max_attempts: u32,
    pub auto_lock_seconds: u64,
}

impl Default for LockPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            auto_lock_seconds: 900,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockState {
    pub enabled: bool,
    pub salt_b64: String,
    pub hash_b64: String,
    pub failed_attempts: u32,
    pub locked_until_epoch: Option<u64>,
    pub unlocked_at_epoch: Option<u64>,
    pub policy: LockPolicy,
}

pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn create_lock_state(password: &str, policy: LockPolicy) -> Result<LockState, ProfileError> {
    if password.len() < 8 {
        return Err(ProfileError::Validation(
            "password must be at least 8 characters".to_string(),
        ));
    }
    let mut salt = [0u8; LOCK_SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let hash = hash_password(password, &salt);
    Ok(LockState {
        enabled: true,
        salt_b64: B64.encode(salt),
        hash_b64: B64.encode(hash),
        failed_attempts: 0,
        locked_until_epoch: None,
        unlocked_at_epoch: None,
        policy,
    })
}

pub fn verify_and_update(
    state: &mut LockState,
    password: &str,
    profile_id: &str,
) -> Result<bool, ProfileError> {
    if !state.enabled {
        return Ok(true);
    }
    let now = now_epoch();
    if let Some(until) = state.locked_until_epoch {
        if now < until {
            return Err(ProfileError::UnlockAttemptsExceeded(profile_id.to_string()));
        }
    }

    let salt = B64
        .decode(&state.salt_b64)
        .map_err(|e| ProfileError::Crypto(format!("invalid lock salt: {e}")))?;
    let expected = B64
        .decode(&state.hash_b64)
        .map_err(|e| ProfileError::Crypto(format!("invalid lock hash: {e}")))?;
    let actual = hash_password(password, &salt);

    if expected == actual {
        state.failed_attempts = 0;
        state.locked_until_epoch = None;
        state.unlocked_at_epoch = Some(now);
        return Ok(true);
    }

    state.failed_attempts += 1;
    if state.failed_attempts >= state.policy.max_attempts {
        state.locked_until_epoch = Some(now + state.policy.auto_lock_seconds);
        state.failed_attempts = 0;
        return Err(ProfileError::UnlockAttemptsExceeded(profile_id.to_string()));
    }
    Ok(false)
}

pub fn is_unlock_expired(state: &LockState) -> bool {
    if !state.enabled {
        return false;
    }
    let Some(unlocked_at) = state.unlocked_at_epoch else {
        return true;
    };
    let now = now_epoch();
    now.saturating_sub(unlocked_at) > state.policy.auto_lock_seconds
}

fn hash_password(password: &str, salt: &[u8]) -> [u8; LOCK_HASH_LEN] {
    let mut out = [0u8; LOCK_HASH_LEN];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, LOCK_ROUNDS, &mut out);
    out
}
