//! API keys in Windows Credential Manager (per-user, protected by the Windows login).

use vex_core::secrets::SecretStore;

const SERVICE: &str = "VEX";

pub struct CredentialManager;

fn entry(name: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, name).map_err(|e| format!("Windows Credential Manager is unavailable: {e}"))
}

impl SecretStore for CredentialManager {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        match entry(name)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("Couldn't read the saved key: {e}")),
        }
    }
    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        entry(name)?.set_password(value).map_err(|e| format!("Couldn't save the key to Windows Credential Manager: {e}"))
    }
    fn delete(&self, name: &str) -> Result<(), String> {
        match entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("Couldn't remove the saved key: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs against the real Credential Manager (CI runs it on a Windows runner with `--ignored`).
    #[test]
    #[ignore = "touches the real Windows Credential Manager"]
    fn credential_manager_round_trip() {
        let s = CredentialManager;
        let name = "vex-test-credential";
        s.delete(name).unwrap();
        assert_eq!(s.get(name).unwrap(), None);
        s.set(name, "sk-test-value-123").unwrap();
        assert_eq!(s.get(name).unwrap().as_deref(), Some("sk-test-value-123"));
        s.set(name, "replaced").unwrap();
        assert_eq!(s.get(name).unwrap().as_deref(), Some("replaced"));
        s.delete(name).unwrap();
        assert_eq!(s.get(name).unwrap(), None);
    }
}
