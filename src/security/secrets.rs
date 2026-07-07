use aes_gcm::aead::{Aead, OsRng, rand_core::RngCore};
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::RwLock,
};

const MARKER: &str = "enc:v1:";

#[derive(Clone)]
pub struct SecretBox {
    cipher: Aes256Gcm,
}

pub struct SecretManager {
    path: PathBuf,
    current: RwLock<SecretBox>,
}

impl SecretBox {
    pub fn from_app_secret(secret: &str) -> Self {
        let key_bytes = derive_key(secret);
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        Self {
            cipher: Aes256Gcm::new(key),
        }
    }

    /// Encrypt a plaintext value and return `enc:v1:<base64(nonce|ciphertext)>`.
    /// Empty input passes through unchanged so blank fields remain blank in the UI.
    pub fn encrypt(&self, plaintext: &str) -> String {
        if plaintext.is_empty() || is_encrypted(plaintext) {
            return plaintext.to_string();
        }
        let mut nonce = [0_u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = match self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
        {
            Ok(value) => value,
            Err(_) => return plaintext.to_string(),
        };
        let mut blob = Vec::with_capacity(nonce.len() + ciphertext.len());
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);
        format!("{}{}", MARKER, STANDARD.encode(&blob))
    }

    /// Decrypt a value produced by `encrypt`. If the value doesn't carry the marker
    /// it's returned as-is (legacy plaintext still works).
    pub fn decrypt(&self, value: &str) -> String {
        let Some(payload) = value.strip_prefix(MARKER) else {
            return value.to_string();
        };
        let Ok(blob) = STANDARD.decode(payload) else {
            return String::new();
        };
        if blob.len() < 12 {
            return String::new();
        }
        let (nonce, ciphertext) = blob.split_at(12);
        match self.cipher.decrypt(Nonce::from_slice(nonce), ciphertext) {
            Ok(bytes) => String::from_utf8(bytes).unwrap_or_default(),
            Err(_) => String::new(),
        }
    }
}

impl SecretManager {
    pub fn load_or_create(
        path: PathBuf,
        legacy_secret: &str,
    ) -> anyhow::Result<(Self, SecretBox, bool)> {
        let legacy_box = SecretBox::from_app_secret(legacy_secret);
        let mut created = false;
        let secret = match fs::read_to_string(&path) {
            Ok(value) if !value.trim().is_empty() => value.trim().to_string(),
            _ => {
                let secret = generate_secret();
                write_secret_file(&path, &secret)?;
                created = true;
                secret
            }
        };
        Ok((
            Self {
                path,
                current: RwLock::new(SecretBox::from_app_secret(&secret)),
            },
            legacy_box,
            created,
        ))
    }

    pub fn encrypt(&self, plaintext: &str) -> String {
        self.current
            .read()
            .map(|box_| box_.encrypt(plaintext))
            .unwrap_or_else(|_| plaintext.to_string())
    }

    pub fn decrypt(&self, value: &str) -> String {
        self.current
            .read()
            .map(|box_| box_.decrypt(value))
            .unwrap_or_default()
    }

    pub fn current_box(&self) -> anyhow::Result<SecretBox> {
        self.current
            .read()
            .map(|box_| box_.clone())
            .map_err(|_| anyhow::anyhow!("secret manager lock poisoned"))
    }

    pub fn key_path(&self) -> &Path {
        &self.path
    }

    pub fn random_secret_box() -> (String, SecretBox) {
        let secret = generate_secret();
        let box_ = SecretBox::from_app_secret(&secret);
        (secret, box_)
    }

    pub fn install_secret(&self, secret: &str, box_: SecretBox) -> anyhow::Result<()> {
        write_secret_file(&self.path, &secret)?;
        *self
            .current
            .write()
            .map_err(|_| anyhow::anyhow!("secret manager lock poisoned"))? = box_;
        Ok(())
    }
}

pub fn is_encrypted(value: &str) -> bool {
    value.starts_with(MARKER)
}

fn derive_key(secret: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"dujiao-rust/v1/");
    hasher.update(secret.as_bytes());
    hasher.finalize().into()
}

fn generate_secret() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    STANDARD.encode(bytes)
}

fn write_secret_file(path: &Path, secret: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{secret}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let sb = SecretBox::from_app_secret("hunter2");
        let secret = "smtp-pwd-1234";
        let enc = sb.encrypt(secret);
        assert!(enc.starts_with(MARKER));
        assert_eq!(sb.decrypt(&enc), secret);
    }

    #[test]
    fn empty_passthrough() {
        let sb = SecretBox::from_app_secret("k");
        assert_eq!(sb.encrypt(""), "");
        assert_eq!(sb.decrypt(""), "");
    }

    #[test]
    fn legacy_plaintext_decrypt_passthrough() {
        let sb = SecretBox::from_app_secret("k");
        assert_eq!(sb.decrypt("plain"), "plain");
    }

    #[test]
    fn encrypt_idempotent() {
        let sb = SecretBox::from_app_secret("k");
        let once = sb.encrypt("x");
        assert_eq!(sb.encrypt(&once), once);
    }
}
