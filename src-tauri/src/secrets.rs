//! Chooses where API keys are stored.

use std::sync::Arc;
use vex_core::secrets::SecretStore;

/// Windows: Credential Manager. Other platforms (development builds only): memory for the session.
pub fn default_store() -> Arc<dyn SecretStore> {
    #[cfg(windows)]
    {
        Arc::new(crate::secrets_windows::CredentialManager)
    }
    #[cfg(not(windows))]
    {
        Arc::new(vex_core::secrets::MemoryStore::default())
    }
}
