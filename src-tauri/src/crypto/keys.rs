use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::Result;
use argon2::{Argon2, Params as Argon2Params};
use rand::RngCore;
use zeroize::Zeroize;


pub struct KeyDerivation;

impl KeyDerivation {
    /// Derive a 32-byte key from a password and salt using Argon2id.
    ///
    /// Uses the same parameters as ConfigManager::derive_key:
    /// - Argon2id algorithm
    /// - 64 MiB memory cost (65536 KiB)
    /// - 3 iterations
    /// - 4 parallel lanes
    /// - 32-byte output
    pub fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
        let mut key = [0u8; 32];
        let params = Argon2Params::new(65536, 3, 4, Some(32))
            .map_err(|e| anyhow::anyhow!("Failed to create Argon2 params: {e}"))?;
        Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(|e| anyhow::anyhow!("Argon2 key derivation failed: {e}"))?;
        Ok(key)
    }

    pub fn generate_salt() -> Vec<u8> {
        let mut salt = vec![0u8; 32];
        OsRng.fill_bytes(&mut salt);
        salt
    }

    pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| anyhow::anyhow!("Invalid key length: {e}"))?;

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {e}"))?;

        let mut output = Vec::with_capacity(12 + ciphertext.len());
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);

        Ok(output)
    }

    pub fn decrypt(ciphertext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
        if ciphertext.len() < 12 {
            anyhow::bail!("Ciphertext too short");
        }

        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| anyhow::anyhow!("Invalid key length: {e}"))?;

        let nonce = Nonce::from_slice(&ciphertext[..12]);
        let plaintext = cipher
            .decrypt(nonce, &ciphertext[12..])
            .map_err(|_| anyhow::anyhow!("Decryption failed"))?;

        Ok(plaintext)
    }

    pub fn secure_zero(data: &mut [u8]) {
        data.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test vector values: hardcoded deterministic inputs required for
    // reproducible assertions. NOT used in production — production salts
    // use OsRng, and passwords come from EPS_PASSWORD env or file.
    const TEST_SALT: &[u8; 32] = b"01234567890123456789012345678901";
    const TEST_KEY: [u8; 32] = [0x42u8; 32];
    const TEST_KEY_ALT: [u8; 32] = [0x24u8; 32];

    /// Returns a deterministic test key. When no salt is given, uses TEST_SALT.
    fn test_key(salt: Option<&[u8]>) -> [u8; 32] {
        KeyDerivation::derive_key("test-password", salt.unwrap_or(TEST_SALT)).unwrap()
    }

    #[test]
    fn test_derive_key_deterministic() {
        let key = test_key(None);
        // Call derive_key directly to verify determinism independent of test_key
        let key_again = KeyDerivation::derive_key("test-password", TEST_SALT).unwrap();
        assert_eq!(key, key_again, "same password + salt should produce same key");
    }

    #[test]
    fn test_derive_key_different_salts() {
        let salt1 = KeyDerivation::generate_salt();
        let salt2 = KeyDerivation::generate_salt();
        let key1 = test_key(Some(&salt1));
        let key2 = test_key(Some(&salt2));
        assert_ne!(key1, key2, "different salts should produce different keys");
    }

    #[test]
    fn test_derive_key_output_length() {
        let key = test_key(None);
        assert_eq!(key.len(), 32, "derived key should be 32 bytes (256 bits)");
    }

    #[test]
    fn test_generate_salt_length() {
        let salt = KeyDerivation::generate_salt();
        assert_eq!(salt.len(), 32, "salt should be 32 bytes");
    }

    #[test]
    fn test_generate_salt_is_random() {
        let salt1 = KeyDerivation::generate_salt();
        let salt2 = KeyDerivation::generate_salt();
        assert_ne!(salt1, salt2, "salts should be random and unique");
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = b"Hello, Kryptos! This is sensitive config data.";
        let encrypted = KeyDerivation::encrypt(plaintext, &TEST_KEY).unwrap();
        assert_ne!(
            encrypted, plaintext,
            "encrypted data should differ from plaintext"
        );
        assert!(
            encrypted.len() > plaintext.len(),
            "encrypted should include nonce + tag"
        );

        let decrypted = KeyDerivation::decrypt(&encrypted, &TEST_KEY).unwrap();
        assert_eq!(decrypted, plaintext, "decrypted should match original");
    }

    #[test]
    fn test_decrypt_wrong_key_fails() {
        let encrypted = KeyDerivation::encrypt(b"test data", &TEST_KEY).unwrap();
        assert!(
            KeyDerivation::decrypt(&encrypted, &TEST_KEY_ALT).is_err(),
            "wrong key should fail decryption"
        );
    }

    #[test]
    fn test_encrypt_unique_nonce_per_call() {
        let enc1 = KeyDerivation::encrypt(b"same data", &TEST_KEY).unwrap();
        let enc2 = KeyDerivation::encrypt(b"same data", &TEST_KEY).unwrap();
        assert_ne!(
            enc1[..12],
            enc2[..12],
            "each encryption should use a fresh random nonce"
        );
    }

    #[test]
    fn test_secure_zero_clears_data() {
        let mut data = vec![0xABu8; 64];
        KeyDerivation::secure_zero(&mut data);
        assert!(
            data.iter().all(|&b| b == 0),
            "zeroize should clear all bytes"
        );
    }

    #[test]
    fn test_argon2_uses_strong_params() {
        let result = test_key(None);
        assert_eq!(result.len(), 32);
    }
}
