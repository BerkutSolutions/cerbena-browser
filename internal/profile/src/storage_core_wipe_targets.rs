use super::*;

pub(crate) fn wipe_targets_for_type(root: &Path, ty: WipeDataType, scopes: &[String]) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    let engine_root = root.join("engine-profile");
    match ty {
        WipeDataType::Cookies => {
            targets.push(root.join("data").join("cookies"));
            for scope in scopes {
                targets.push(root.join("data").join("cookies").join(scope));
            }
            targets.push(engine_root.join("Default").join("Network").join("Cookies"));
            targets.push(
                engine_root
                    .join("Default")
                    .join("Network")
                    .join("Cookies-journal"),
            );
            targets.push(engine_root.join("Default").join("Cookies"));
            targets.push(engine_root.join("Default").join("Cookies-journal"));
            targets.push(engine_root.join("cookies.sqlite"));
            targets.push(engine_root.join("cookies.sqlite-wal"));
            targets.push(engine_root.join("cookies.sqlite-shm"));
        }
        WipeDataType::History => {
            targets.push(root.join("data").join("history"));
            for scope in scopes {
                targets.push(root.join("data").join("history").join(scope));
            }
            targets.push(engine_root.join("Default").join("History"));
            targets.push(engine_root.join("Default").join("History-journal"));
            targets.push(engine_root.join("places.sqlite"));
            targets.push(engine_root.join("places.sqlite-wal"));
            targets.push(engine_root.join("places.sqlite-shm"));
        }
        WipeDataType::Passwords => {
            targets.push(root.join("data").join("passwords"));
            targets.push(engine_root.join("Default").join("Login Data"));
            targets.push(engine_root.join("Default").join("Login Data For Account"));
            targets.push(engine_root.join("Default").join("Login Data-journal"));
            targets.push(engine_root.join("logins.json"));
            targets.push(engine_root.join("key4.db"));
        }
        WipeDataType::Cache => {
            targets.push(root.join("cache"));
            targets.push(engine_root.join("Default").join("Cache"));
            targets.push(engine_root.join("Default").join("Code Cache"));
            targets.push(engine_root.join("Default").join("GPUCache"));
            targets.push(
                engine_root
                    .join("Default")
                    .join("Service Worker")
                    .join("CacheStorage"),
            );
            targets.push(engine_root.join("cache2"));
            targets.push(engine_root.join("startupCache"));
            targets.push(engine_root.join("shader-cache"));
        }
        WipeDataType::ExtensionsStorage => {
            targets.push(root.join("extensions"));
            targets.push(engine_root.join("Default").join("Local Extension Settings"));
            targets.push(engine_root.join("Default").join("Extension State"));
            targets.push(engine_root.join("storage").join("default"));
            targets.push(engine_root.join("browser-extension-data"));
        }
    }
    targets
}

impl ProfileStorage {
    pub(crate) fn cleanup_cache_dir(&self, cache: &Path) -> Result<CacheCleanupResult, ProfileError> {
        if !cache.exists() {
            return Ok(CacheCleanupResult::default());
        }
        let mut result = CacheCleanupResult::default();
        for entry in fs::read_dir(cache)? {
            let entry = entry?;
            let p = entry.path();
            let removed = if entry.file_type()?.is_dir() {
                let res: Result<(), ProfileError> = (|| {
                    self.wipe_tree(&p)?;
                    fs::remove_dir_all(&p)?;
                    Ok(())
                })();
                res.is_ok()
            } else {
                self.wipe_file(&p).is_ok()
            };
            if removed {
                result.removed_entries += 1;
            } else {
                result.errors += 1;
            }
        }
        Ok(result)
    }
}
