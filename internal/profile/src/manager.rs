use std::path::Path;

use uuid::Uuid;

use crate::{
    audit::{AuditFilter, AuditLogger},
    crypto::{decrypt_blob, encrypt_blob, CRYPTO_VERSION},
    errors::ProfileError,
    lock::{create_lock_state, is_unlock_expired, verify_and_update, LockPolicy},
    model::{
        validate_name, validate_tags, CreateProfileInput, PatchProfileInput, ProfileMetadata,
        ProfileState,
    },
    storage::ProfileStorage,
    wipe::SelectiveWipeRequest,
    CacheCleanupResult,
};

#[derive(Debug, Clone)]
pub struct ProfileManager {
    storage: ProfileStorage,
    audit: AuditLogger,
}

impl ProfileManager {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let storage = ProfileStorage::new(root)?;
        let audit = AuditLogger::new(storage.root().join("audit").join("events.jsonl"))?;
        Ok(Self { storage, audit })
    }

    pub fn create_profile(
        &self,
        input: CreateProfileInput,
    ) -> Result<ProfileMetadata, ProfileError> {
        validate_name(&input.name)?;
        validate_tags(&input.tags)?;

        let now = utc_now();
        let profile = ProfileMetadata {
            id: Uuid::new_v4(),
            name: input.name.trim().to_string(),
            description: input.description,
            tags: input.tags,
            engine: input.engine,
            state: ProfileState::Created,
            default_start_page: input.default_start_page,
            default_search_provider: input.default_search_provider,
            ephemeral_mode: input.ephemeral_mode,
            password_lock_enabled: input.password_lock_enabled,
            panic_frame_enabled: input.panic_frame_enabled,
            panic_frame_color: input.panic_frame_color,
            panic_protected_sites: input.panic_protected_sites,
            crypto_version: CRYPTO_VERSION,
            ephemeral_retain_paths: input.ephemeral_retain_paths,
            created_at: now.clone(),
            updated_at: now,
        };

        self.storage.create_layout(profile.id)?;
        if let Err(err) = self.storage.write_metadata_atomic(&profile) {
            let _ = self.storage.secure_delete_profile(profile.id);
            return Err(err);
        }
        self.audit
            .record_system("profile.create", profile.id, "success")?;
        Ok(profile)
    }

    pub fn get_profile(&self, profile_id: Uuid) -> Result<ProfileMetadata, ProfileError> {
        self.storage.read_metadata(profile_id)
    }

    pub fn list_profiles(&self) -> Result<Vec<ProfileMetadata>, ProfileError> {
        let mut list = Vec::new();
        for id in self.storage.list_profile_ids()? {
            list.push(self.storage.read_metadata(id)?);
        }
        list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(list)
    }

    pub fn update_profile(
        &self,
        profile_id: Uuid,
        patch: PatchProfileInput,
    ) -> Result<ProfileMetadata, ProfileError> {
        self.update_profile_with_actor(profile_id, patch, None, "system")
    }

    pub fn update_profile_with_actor(
        &self,
        profile_id: Uuid,
        patch: PatchProfileInput,
        expected_updated_at: Option<&str>,
        actor: &str,
    ) -> Result<ProfileMetadata, ProfileError> {
        let mut profile = self.storage.read_metadata(profile_id)?;
        if let Some(version) = expected_updated_at {
            if profile.updated_at != version {
                return Err(ProfileError::Conflict(
                    "profile changed by another update".to_string(),
                ));
            }
        }
        if profile.state == ProfileState::Running
            && (patch.ephemeral_mode.is_some()
                || patch.password_lock_enabled.is_some()
                || patch.panic_frame_enabled.is_some())
        {
            return Err(ProfileError::Conflict(
                "cannot change security flags while profile is running".to_string(),
            ));
        }

        if let Some(name) = patch.name {
            validate_name(&name)?;
            profile.name = name.trim().to_string();
        }
        if let Some(description) = patch.description {
            profile.description = description;
        }
        if let Some(tags) = patch.tags {
            validate_tags(&tags)?;
            profile.tags = tags;
        }
        if let Some(state) = patch.state {
            profile.state = state;
        }
        if let Some(page) = patch.default_start_page {
            profile.default_start_page = page;
        }
        if let Some(provider) = patch.default_search_provider {
            profile.default_search_provider = provider;
        }
        if let Some(ephemeral_mode) = patch.ephemeral_mode {
            profile.ephemeral_mode = ephemeral_mode;
        }
        if let Some(password_lock_enabled) = patch.password_lock_enabled {
            profile.password_lock_enabled = password_lock_enabled;
        }
        if let Some(panic_frame_enabled) = patch.panic_frame_enabled {
            profile.panic_frame_enabled = panic_frame_enabled;
        }
        if let Some(panic_frame_color) = patch.panic_frame_color {
            profile.panic_frame_color = panic_frame_color;
        }
        if let Some(panic_protected_sites) = patch.panic_protected_sites {
            profile.panic_protected_sites = panic_protected_sites;
        }
        if let Some(ephemeral_retain_paths) = patch.ephemeral_retain_paths {
            profile.ephemeral_retain_paths = ephemeral_retain_paths;
        }

        profile.updated_at = utc_now();
        self.storage.write_metadata_atomic(&profile)?;
        self.audit
            .record("profile.update", actor, profile_id, "success", None)?;
        Ok(profile)
    }

    pub fn delete_profile(&self, profile_id: Uuid) -> Result<(), ProfileError> {
        self.delete_profile_with_actor(profile_id, "system")
    }

    pub fn delete_profile_with_actor(
        &self,
        profile_id: Uuid,
        actor: &str,
    ) -> Result<(), ProfileError> {
        self.storage.secure_delete_profile(profile_id)?;
        self.audit
            .record("profile.delete", actor, profile_id, "success", None)?;
        Ok(())
    }

    pub fn set_profile_password(
        &self,
        profile_id: Uuid,
        password: &str,
        policy: Option<LockPolicy>,
    ) -> Result<(), ProfileError> {
        let mut profile = self.storage.read_metadata(profile_id)?;
        let lock_state = create_lock_state(password, policy.unwrap_or_default())?;
        self.storage.write_lock_state(profile_id, &lock_state)?;
        profile.password_lock_enabled = true;
        profile.state = ProfileState::Locked;
        profile.updated_at = utc_now();
        self.storage.write_metadata_atomic(&profile)?;
        self.audit
            .record_system("profile.lock.set", profile_id, "success")?;
        Ok(())
    }

    pub fn unlock_profile(&self, profile_id: Uuid, password: &str) -> Result<bool, ProfileError> {
        let mut lock_state = self.storage.read_lock_state(profile_id)?;
        let verified = verify_and_update(&mut lock_state, password, &profile_id.to_string())?;
        self.storage.write_lock_state(profile_id, &lock_state)?;
        if verified {
            let mut profile = self.storage.read_metadata(profile_id)?;
            profile.state = ProfileState::Ready;
            profile.updated_at = utc_now();
            self.storage.write_metadata_atomic(&profile)?;
            self.audit
                .record_system("profile.unlock", profile_id, "success")?;
        } else {
            self.audit
                .record_system("profile.unlock", profile_id, "failed")?;
        }
        Ok(verified)
    }

    pub fn ensure_unlocked(&self, profile_id: Uuid) -> Result<(), ProfileError> {
        let profile = self.storage.read_metadata(profile_id)?;
        if !profile.password_lock_enabled {
            return Ok(());
        }
        let lock_state = self.storage.read_lock_state(profile_id)?;
        if is_unlock_expired(&lock_state) {
            return Err(ProfileError::Locked(profile_id.to_string()));
        }
        Ok(())
    }

    pub fn encrypt_profile_secret(
        &self,
        profile_id: Uuid,
        secret_key: &str,
        plaintext: &[u8],
    ) -> Result<(), ProfileError> {
        self.ensure_unlocked(profile_id)?;
        let blob = encrypt_blob(&profile_id.to_string(), secret_key, plaintext)?;
        self.storage.write_encrypted_secrets(profile_id, &blob)?;
        self.audit
            .record_system("profile.secret.encrypt", profile_id, "success")?;
        Ok(())
    }

    pub fn decrypt_profile_secret(
        &self,
        profile_id: Uuid,
        secret_key: &str,
    ) -> Result<Vec<u8>, ProfileError> {
        self.ensure_unlocked(profile_id)?;
        let blob = self.storage.read_encrypted_secrets(profile_id)?;
        decrypt_blob(&profile_id.to_string(), secret_key, &blob)
    }

    pub fn close_profile(&self, profile_id: Uuid) -> Result<(), ProfileError> {
        let mut profile = self.storage.read_metadata(profile_id)?;
        self.storage.close_profile_with_ephemeral_policy(&profile)?;
        if profile.password_lock_enabled {
            profile.state = ProfileState::Locked;
        } else {
            profile.state = ProfileState::Stopped;
        }
        profile.updated_at = utc_now();
        self.storage.write_metadata_atomic(&profile)?;
        self.audit
            .record_system("profile.close", profile_id, "success")?;
        Ok(())
    }

    pub fn get_audit_events(
        &self,
        filter: AuditFilter,
    ) -> Result<Vec<crate::audit::AuditEvent>, ProfileError> {
        self.audit.read_events(filter)
    }

    pub fn selective_wipe_profile_data(
        &self,
        profile_id: Uuid,
        request: &SelectiveWipeRequest,
        actor: &str,
    ) -> Result<Vec<String>, ProfileError> {
        self.ensure_unlocked(profile_id)?;
        let affected = self.storage.selective_wipe(profile_id, request)?;
        self.audit.record(
            "profile.selective_wipe",
            actor,
            profile_id,
            "success",
            Some(&format!("affected_paths={}", affected.len())),
        )?;
        Ok(affected)
    }

    pub fn cleanup_profile_cache(
        &self,
        profile_id: Uuid,
        actor: &str,
    ) -> Result<CacheCleanupResult, ProfileError> {
        let result = self.storage.cleanup_profile_cache(profile_id)?;
        self.audit.record(
            "profile.cache.cleanup",
            actor,
            profile_id,
            "success",
            Some(&format!("removed={}", result.removed_entries)),
        )?;
        Ok(result)
    }

    pub fn cleanup_all_caches(&self, actor: &str) -> Result<CacheCleanupResult, ProfileError> {
        let result = self.storage.cleanup_all_caches()?;
        self.audit.record(
            "profile.cache.cleanup_all",
            actor,
            Uuid::nil(),
            "success",
            Some(&format!("removed={}", result.removed_entries)),
        )?;
        Ok(result)
    }
}

pub(crate) fn utc_now() -> String {
    let now = std::time::SystemTime::now();
    let ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{ms}Z")
}
