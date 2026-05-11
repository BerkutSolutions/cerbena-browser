use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{
    cache::CacheCleanupResult,
    crypto::EncryptedBlob,
    errors::ProfileError,
    lock::LockState,
    model::ProfileMetadata,
    sqlite_retention::{normalize_site_scopes, retain_scoped_engine_data},
    wipe::{SelectiveWipeRequest, WipeDataType},
};

#[derive(Debug, Clone)]
pub struct ProfileStorage {
    root: PathBuf,
}

impl ProfileStorage {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn profile_dir(&self, profile_id: Uuid) -> Result<PathBuf, ProfileError> {
        let dir = self.root.join(profile_id.to_string());
        self.ensure_in_root(&dir)?;
        Ok(dir)
    }

    pub fn metadata_path(&self, profile_id: Uuid) -> Result<PathBuf, ProfileError> {
        Ok(self.profile_dir(profile_id)?.join("metadata.json"))
    }

    pub fn lock_state_path(&self, profile_id: Uuid) -> Result<PathBuf, ProfileError> {
        Ok(self.profile_dir(profile_id)?.join("lock_state.json"))
    }

    pub fn secrets_path(&self, profile_id: Uuid) -> Result<PathBuf, ProfileError> {
        Ok(self
            .profile_dir(profile_id)?
            .join("data")
            .join("secrets.enc.json"))
    }

    pub fn create_layout(&self, profile_id: Uuid) -> Result<PathBuf, ProfileError> {
        let profile_dir = self.profile_dir(profile_id)?;
        if profile_dir.exists() {
            return Err(ProfileError::AlreadyExists(profile_id.to_string()));
        }
        fs::create_dir_all(profile_dir.join("data"))?;
        fs::create_dir_all(profile_dir.join("cache"))?;
        fs::create_dir_all(profile_dir.join("extensions"))?;
        fs::create_dir_all(profile_dir.join("tmp"))?;
        Ok(profile_dir)
    }

    pub fn write_metadata_atomic(&self, profile: &ProfileMetadata) -> Result<(), ProfileError> {
        let metadata_path = self.metadata_path(profile.id)?;
        let parent = metadata_path
            .parent()
            .ok_or_else(|| ProfileError::InvalidPath(metadata_path.clone()))?;
        fs::create_dir_all(parent)?;

        let tmp_name = format!("metadata.json.tmp-{}", Uuid::new_v4());
        let tmp_path = parent.join(tmp_name);

        let payload = serde_json::to_vec_pretty(profile)?;
        {
            let mut file = File::create(&tmp_path)?;
            file.write_all(&payload)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
        }

        if metadata_path.exists() {
            fs::remove_file(&metadata_path)?;
        }
        fs::rename(&tmp_path, &metadata_path)?;
        Ok(())
    }

    pub fn read_metadata(&self, profile_id: Uuid) -> Result<ProfileMetadata, ProfileError> {
        let path = self.metadata_path(profile_id)?;
        if !path.exists() {
            return Err(ProfileError::NotFound(profile_id.to_string()));
        }
        let mut buf = String::new();
        File::open(path)?.read_to_string(&mut buf)?;
        Ok(serde_json::from_str(&buf)?)
    }

    pub fn write_lock_state(
        &self,
        profile_id: Uuid,
        state: &LockState,
    ) -> Result<(), ProfileError> {
        let path = self.lock_state_path(profile_id)?;
        self.write_json_atomic(&path, state)
    }

    pub fn read_lock_state(&self, profile_id: Uuid) -> Result<LockState, ProfileError> {
        let path = self.lock_state_path(profile_id)?;
        if !path.exists() {
            return Err(ProfileError::NotFound(path.to_string_lossy().to_string()));
        }
        let mut buf = String::new();
        File::open(path)?.read_to_string(&mut buf)?;
        Ok(serde_json::from_str(&buf)?)
    }

    pub fn write_encrypted_secrets(
        &self,
        profile_id: Uuid,
        blob: &EncryptedBlob,
    ) -> Result<(), ProfileError> {
        let path = self.secrets_path(profile_id)?;
        self.write_json_atomic(&path, blob)
    }

    pub fn read_encrypted_secrets(&self, profile_id: Uuid) -> Result<EncryptedBlob, ProfileError> {
        let path = self.secrets_path(profile_id)?;
        if !path.exists() {
            return Err(ProfileError::NotFound(path.to_string_lossy().to_string()));
        }
        let mut buf = String::new();
        File::open(path)?.read_to_string(&mut buf)?;
        Ok(serde_json::from_str(&buf)?)
    }

    pub fn close_profile_with_ephemeral_policy(
        &self,
        profile: &ProfileMetadata,
    ) -> Result<(), ProfileError> {
        if !profile.ephemeral_mode {
            return Ok(());
        }
        let root = self.profile_dir(profile.id)?;
        let cache = root.join("cache");
        if cache.exists() {
            self.wipe_tree(&cache)?;
            fs::remove_dir_all(&cache)?;
            fs::create_dir_all(&cache)?;
        }

        let data = root.join("data");
        if data.exists() {
            let retain = profile
                .ephemeral_retain_paths
                .iter()
                .map(|p| data.join(p))
                .collect::<Vec<_>>();
            self.wipe_tree_except(&data, &retain)?;
        }
        Ok(())
    }

    pub fn selective_wipe(
        &self,
        profile_id: Uuid,
        request: &SelectiveWipeRequest,
    ) -> Result<Vec<String>, ProfileError> {
        let root = self.profile_dir(profile_id)?;
        let mut affected = Vec::new();
        let site_scopes = normalize_site_scopes(&request.site_scopes);
        for data_type in &request.data_types {
            let preserved_engine_targets =
                retain_scoped_engine_data(&root, *data_type, &site_scopes)?;
            let targets = wipe_targets_for_type(&root, *data_type, &request.site_scopes);
            for target in targets {
                if !target.exists() {
                    continue;
                }
                if preserved_engine_targets.iter().any(|path| path == &target) {
                    affected.push(target.to_string_lossy().to_string());
                    continue;
                }
                let keep_paths = request
                    .retain_paths
                    .iter()
                    .map(|p| root.join(p))
                    .collect::<Vec<_>>();
                if keep_paths.iter().any(|p| p == &target) {
                    continue;
                }
                if target.is_dir() {
                    self.wipe_tree_except(&target, &keep_paths)?;
                    let _ = fs::remove_dir_all(&target);
                    fs::create_dir_all(&target)?;
                } else {
                    self.wipe_file(&target)?;
                }
                affected.push(target.to_string_lossy().to_string());
            }
        }
        Ok(affected)
    }

    pub fn list_profile_ids(&self) -> Result<Vec<Uuid>, ProfileError> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Ok(id) = Uuid::parse_str(&name) {
                ids.push(id);
            }
        }
        ids.sort_unstable();
        Ok(ids)
    }

    pub fn cleanup_profile_cache(
        &self,
        profile_id: Uuid,
    ) -> Result<CacheCleanupResult, ProfileError> {
        let cache = self.profile_dir(profile_id)?.join("cache");
        self.cleanup_cache_dir(&cache)
    }

    pub fn cleanup_all_caches(&self) -> Result<CacheCleanupResult, ProfileError> {
        let mut total = CacheCleanupResult::default();
        for id in self.list_profile_ids()? {
            let item = self.cleanup_profile_cache(id)?;
            total.removed_entries += item.removed_entries;
            total.errors += item.errors;
        }
        Ok(total)
    }

    pub fn secure_delete_profile(&self, profile_id: Uuid) -> Result<(), ProfileError> {
        let dir = self.profile_dir(profile_id)?;
        if !dir.exists() {
            return Err(ProfileError::NotFound(profile_id.to_string()));
        }
        self.wipe_tree(&dir)?;
        fs::remove_dir_all(&dir)?;
        Ok(())
    }

    fn wipe_tree(&self, path: &Path) -> Result<(), ProfileError> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            if entry.file_type()?.is_dir() {
                self.wipe_tree(&entry_path)?;
                continue;
            }
            self.wipe_file(&entry_path)?;
        }
        Ok(())
    }

    fn wipe_tree_except(&self, path: &Path, keep_paths: &[PathBuf]) -> Result<(), ProfileError> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            if keep_paths.iter().any(|k| k == &entry_path) {
                continue;
            }
            if entry.file_type()?.is_dir() {
                self.wipe_tree(&entry_path)?;
                fs::remove_dir_all(&entry_path)?;
                continue;
            }
            self.wipe_file(&entry_path)?;
        }
        Ok(())
    }

    fn wipe_file(&self, path: &Path) -> Result<(), ProfileError> {
        let metadata = fs::metadata(path)?;
        let len = metadata.len();
        if len > 0 {
            let mut file = File::options().write(true).open(path)?;
            let mut left = len;
            let zeros = [0u8; 8192];
            while left > 0 {
                let chunk = left.min(zeros.len() as u64) as usize;
                file.write_all(&zeros[..chunk])?;
                left -= chunk as u64;
            }
            file.sync_all()?;
        }
        fs::remove_file(path)?;
        Ok(())
    }

    fn ensure_in_root(&self, candidate: &Path) -> Result<(), ProfileError> {
        let normalized = candidate
            .components()
            .collect::<PathBuf>()
            .to_string_lossy()
            .to_string();
        if normalized.contains("..") {
            return Err(ProfileError::InvalidPath(candidate.to_path_buf()));
        }
        Ok(())
    }

    fn write_json_atomic<T: serde::Serialize>(
        &self,
        target: &Path,
        value: &T,
    ) -> Result<(), ProfileError> {
        let parent = target
            .parent()
            .ok_or_else(|| ProfileError::InvalidPath(target.to_path_buf()))?;
        fs::create_dir_all(parent)?;
        let tmp_name = format!(
            "{}.tmp-{}",
            target.file_name().unwrap_or_default().to_string_lossy(),
            Uuid::new_v4()
        );
        let tmp_path = parent.join(tmp_name);
        let payload = serde_json::to_vec_pretty(value)?;
        {
            let mut file = File::create(&tmp_path)?;
            file.write_all(&payload)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
        }
        if target.exists() {
            fs::remove_file(target)?;
        }
        fs::rename(&tmp_path, target)?;
        Ok(())
    }
}


#[path = "storage_core_wipe_targets.rs"]
mod support;
use support::*;
