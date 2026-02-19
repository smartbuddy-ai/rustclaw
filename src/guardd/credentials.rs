use aes_gcm::aead::{rand_core::RngCore, Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use zeroize::Zeroize;

/// Secret wrapper that zeroizes on drop.
#[derive(Debug, Clone)]
pub struct Secret(Vec<u8>);

impl Secret {
    /// Create a secret from bytes.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Borrow raw secret bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSecret {
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

/// Encrypted credential store backed by AES-256-GCM.
#[derive(Debug, Clone)]
pub struct CredentialStore {
    path: PathBuf,
    key: Secret,
}

impl CredentialStore {
    /// Create a new credential store at the given path.
    pub fn new(path: PathBuf, key: Secret) -> Result<Self> {
        if key.as_bytes().len() != 32 {
            anyhow::bail!("AES-256-GCM requires a 32-byte key");
        }
        Ok(Self { path, key })
    }

    /// Store a secret under the given name.
    pub fn store(&self, name: &str, secret: &Secret) -> Result<()> {
        let mut map = self.read_store()?;
        let cipher = self.cipher()?;
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, secret.as_bytes())
            .map_err(|e| anyhow::anyhow!("encrypting secret: {}", e))?;
        map.insert(
            name.to_string(),
            StoredSecret {
                nonce: nonce_bytes.to_vec(),
                ciphertext,
            },
        );
        self.write_store(&map)
    }

    /// Retrieve a secret by name.
    pub fn retrieve(&self, name: &str) -> Result<Option<Secret>> {
        let map = self.read_store()?;
        let Some(stored) = map.get(name) else {
            return Ok(None);
        };
        let cipher = self.cipher()?;
        let nonce = Nonce::from_slice(&stored.nonce);
        let plaintext = cipher
            .decrypt(nonce, stored.ciphertext.as_ref())
            .map_err(|e| anyhow::anyhow!("decrypting secret: {}", e))?;
        Ok(Some(Secret::new(plaintext)))
    }

    /// Rotate the encryption key while preserving stored secrets.
    pub fn rotate(&mut self, new_key: Secret) -> Result<()> {
        if new_key.as_bytes().len() != 32 {
            anyhow::bail!("AES-256-GCM requires a 32-byte key");
        }
        let map = self.read_store()?;
        let mut decrypted = HashMap::new();
        for (name, stored) in map {
            let cipher = self.cipher()?;
            let nonce = Nonce::from_slice(&stored.nonce);
            let plaintext = cipher
                .decrypt(nonce, stored.ciphertext.as_ref())
                .map_err(|e| anyhow::anyhow!("decrypting secret during rotate: {}", e))?;
            decrypted.insert(name, plaintext);
        }

        self.key = new_key;
        let cipher = self.cipher()?;
        let mut rotated = HashMap::new();
        for (name, plaintext) in decrypted {
            let mut nonce_bytes = [0u8; 12];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = Nonce::from_slice(&nonce_bytes);
            let ciphertext = cipher
                .encrypt(nonce, plaintext.as_ref())
                .map_err(|e| anyhow::anyhow!("encrypting secret during rotate: {}", e))?;
            rotated.insert(
                name,
                StoredSecret {
                    nonce: nonce_bytes.to_vec(),
                    ciphertext,
                },
            );
        }

        self.write_store(&rotated)
    }

    fn cipher(&self) -> Result<Aes256Gcm> {
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(self.key.as_bytes());
        Ok(Aes256Gcm::new(key))
    }

    fn read_store(&self) -> Result<HashMap<String, StoredSecret>> {
        if !self.path.exists() {
            return Ok(HashMap::new());
        }
        let content = fs::read_to_string(&self.path)
            .with_context(|| format!("reading credential store {}", self.path.display()))?;
        let map = serde_json::from_str(&content).context("parsing credential store")?;
        Ok(map)
    }

    fn write_store(&self, map: &HashMap<String, StoredSecret>) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("creating credential store dir {}", parent.display())
            })?;
        }
        let content = serde_json::to_string_pretty(map).context("encoding credential store")?;

        // Bonus: Atomic write — write to .tmp then rename to prevent corruption on crash
        let tmp_path = self.path.with_extension("json.tmp");
        fs::write(&tmp_path, &content)
            .with_context(|| format!("writing credential store tmp {}", tmp_path.display()))?;

        // Bonus: Set file permissions to 0o600 (user-only read/write)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = fs::set_permissions(&tmp_path, perms);
        }

        fs::rename(&tmp_path, &self.path)
            .with_context(|| format!("renaming credential store {}", self.path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn key_bytes(seed: u8) -> Vec<u8> {
        (seed..seed + 32).collect()
    }

    #[test]
    fn store_and_retrieve_secret() {
        let temp = TempDir::new().expect("tempdir");
        let store = CredentialStore::new(temp.path().join("creds.json"), Secret::new(key_bytes(1)))
            .expect("store");
        let secret = Secret::new(b"token-123".to_vec());
        store.store("api", &secret).expect("store secret");
        let retrieved = store.retrieve("api").expect("retrieve").expect("value");
        assert_eq!(retrieved.as_bytes(), b"token-123");
    }

    #[test]
    fn rotate_preserves_secrets() {
        let temp = TempDir::new().expect("tempdir");
        let mut store =
            CredentialStore::new(temp.path().join("creds.json"), Secret::new(key_bytes(2)))
                .expect("store");
        let secret = Secret::new(b"rotate-me".to_vec());
        store.store("primary", &secret).expect("store secret");
        store
            .rotate(Secret::new(key_bytes(42)))
            .expect("rotate");
        let retrieved = store
            .retrieve("primary")
            .expect("retrieve")
            .expect("value");
        assert_eq!(retrieved.as_bytes(), b"rotate-me");
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let temp = TempDir::new().expect("tempdir");
        let store =
            CredentialStore::new(temp.path().join("creds.json"), Secret::new(key_bytes(3)))
                .expect("store");
        let secret = Secret::new(b"secret".to_vec());
        store.store("api", &secret).expect("store secret");
        let other =
            CredentialStore::new(temp.path().join("creds.json"), Secret::new(key_bytes(4)))
                .expect("store");
        let result = other.retrieve("api");
        assert!(result.is_err());
    }
}
