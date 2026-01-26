use std::fmt::Debug;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

pub use aish_protocol::config_types::AuthMode;

#[cfg(any(test, feature = "test-support"))]
use once_cell::sync::Lazy;
#[cfg(any(test, feature = "test-support"))]
use std::sync::Mutex;
#[cfg(any(test, feature = "test-support"))]
use tempfile::TempDir;

#[derive(Debug, Clone)]
pub struct AishAuth {
    pub mode: AuthMode,
    pub(crate) api_key: Option<String>,
}

impl PartialEq for AishAuth {
    fn eq(&self, other: &Self) -> bool {
        self.mode == other.mode
    }
}

#[cfg(any(test, feature = "test-support"))]
static TEST_AUTH_TEMP_DIRS: Lazy<Mutex<Vec<TempDir>>> = Lazy::new(|| Mutex::new(Vec::new()));

impl AishAuth {
    /// Loads auth information from legacy environment variables. This currently always returns
    /// `None`.
    pub fn from_env() -> Option<AishAuth> {
        load_auth(true)
    }

    pub async fn get_token(&self) -> Result<String, std::io::Error> {
        Ok(self.api_key.clone().unwrap_or_default())
    }

    pub fn from_api_key(api_key: &str) -> Self {
        Self {
            api_key: Some(api_key.to_owned()),
            mode: AuthMode::ApiKey,
        }
    }

    /// Create a dummy auth for testing purposes
    #[cfg(any(test, feature = "test-support"))]
    pub fn create_dummy_auth_for_testing() -> Self {
        Self::from_api_key("test-api-key")
    }
}

fn load_auth(_enable_codex_api_key_env: bool) -> Option<AishAuth> {
    None
}

/// Internal cached auth state.
#[derive(Clone, Debug)]
struct CachedAuth {
    auth: Option<AishAuth>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn from_api_key_creates_valid_auth() {
        let auth = AishAuth::from_api_key("sk-test-key");
        assert_eq!(auth.mode, AuthMode::ApiKey);
        assert_eq!(auth.api_key, Some("sk-test-key".to_string()));
    }
}

/// Central manager providing a single source of truth for legacy
/// environment-derived authentication data. It loads once (or on preference change) and then
/// hands out cloned `AishAuth` values so the rest of the program has a
/// consistent snapshot.
#[derive(Debug)]
pub struct AuthManager {
    codex_home: PathBuf,
    inner: RwLock<CachedAuth>,
    enable_codex_api_key_env: bool,
}

impl AuthManager {
    /// Create a new manager loading the initial auth from legacy environment variables.
    /// Errors loading auth are swallowed; `auth()` will simply return `None`
    /// in that case so callers can treat it as an unauthenticated state.
    pub fn new(codex_home: PathBuf, enable_codex_api_key_env: bool) -> Self {
        let auth = load_auth(enable_codex_api_key_env);
        Self {
            codex_home,
            inner: RwLock::new(CachedAuth { auth }),
            enable_codex_api_key_env,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    #[expect(clippy::expect_used)]
    /// Create an AuthManager with a specific AishAuth, for testing only.
    pub fn from_auth_for_testing(auth: AishAuth) -> Arc<Self> {
        let cached = CachedAuth { auth: Some(auth) };
        let temp_dir = tempfile::tempdir().expect("temp codex home");
        let codex_home = temp_dir.path().to_path_buf();
        TEST_AUTH_TEMP_DIRS
            .lock()
            .expect("lock test codex homes")
            .push(temp_dir);
        Arc::new(Self {
            codex_home,
            inner: RwLock::new(cached),
            enable_codex_api_key_env: false,
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Create an AuthManager with a specific AishAuth and codex home, for testing only.
    pub fn from_auth_for_testing_with_home(auth: AishAuth, codex_home: PathBuf) -> Arc<Self> {
        let cached = CachedAuth { auth: Some(auth) };
        Arc::new(Self {
            codex_home,
            inner: RwLock::new(cached),
            enable_codex_api_key_env: false,
        })
    }

    /// Current cached auth (clone). May be `None` if not logged in or load failed.
    pub fn auth(&self) -> Option<AishAuth> {
        self.inner.read().ok().and_then(|c| c.auth.clone())
    }

    pub fn codex_home(&self) -> &Path {
        &self.codex_home
    }

    /// Force a reload of the auth information from environment. Returns
    /// whether the auth value changed.
    pub fn reload(&self) -> bool {
        let new_auth = load_auth(self.enable_codex_api_key_env);
        if let Ok(mut guard) = self.inner.write() {
            let changed = !AuthManager::auths_equal(&guard.auth, &new_auth);
            guard.auth = new_auth;
            changed
        } else {
            false
        }
    }

    fn auths_equal(a: &Option<AishAuth>, b: &Option<AishAuth>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// Convenience constructor returning an `Arc` wrapper.
    pub fn shared(codex_home: PathBuf, enable_codex_api_key_env: bool) -> Arc<Self> {
        Arc::new(Self::new(codex_home, enable_codex_api_key_env))
    }

    pub fn get_auth_mode(&self) -> Option<AuthMode> {
        self.auth().map(|a| a.mode)
    }
}
