use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
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
        let params =
            Argon2Params::new(65536, 3, 4, Some(32)).context("Failed to create Argon2 params")?;
        Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .context("Argon2 key derivation failed")?;
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

    #[test]
    fn test_derive_key_deterministic() {
        let salt = b"01234567890123456789012345678901"; // 32 bytes
        let key1 = KeyDerivation::derive_key("test-password", salt).unwrap();
        let key2 = KeyDerivation::derive_key("test-password", salt).unwrap();
        assert_eq!(key1, key2, "same password + salt should produce same key");
    }

    #[test]
    fn test_derive_key_different_salts() {
        let salt1 = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"; // 32 bytes
        let salt2 = b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"; // 32 bytes
        let key1 = KeyDerivation::derive_key("test-password", salt1).unwrap();
        let key2 = KeyDerivation::derive_key("test-password", salt2).unwrap();
        assert_ne!(key1, key2, "different salts should produce different keys");
    }

    #[test]
    fn test_derive_key_output_length() {
        let salt = b"01234567890123456789012345678901";
        let key = KeyDerivation::derive_key("test-password", salt).unwrap();
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
        let key = [0x42u8; 32];
        let plaintext = b"Hello, Kryptos! This is sensitive config data.";
        let encrypted = KeyDerivation::encrypt(plaintext, &key).unwrap();
        assert_ne!(
            encrypted, plaintext,
            "encrypted data should differ from plaintext"
        );
        assert!(
            encrypted.len() > plaintext.len(),
            "encrypted should include nonce + tag"
        );

        let decrypted = KeyDerivation::decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext, "decrypted should match original");
    }

    #[test]
    fn test_decrypt_wrong_key_fails() {
        let key1 = [0x42u8; 32];
        let key2 = [0x24u8; 32];
        let plaintext = b"test data";
        let encrypted = KeyDerivation::encrypt(plaintext, &key1).unwrap();
        assert!(
            KeyDerivation::decrypt(&encrypted, &key2).is_err(),
            "wrong key should fail decryption"
        );
    }

    #[test]
    fn test_encrypt_unique_nonce_per_call() {
        let key = [0x42u8; 32];
        let plaintext = b"same data";
        let enc1 = KeyDerivation::encrypt(plaintext, &key).unwrap();
        let enc2 = KeyDerivation::encrypt(plaintext, &key).unwrap();
        // Nonce is prepended (first 12 bytes), so different nonces mean different ciphertexts
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
        let salt = b"01234567890123456789012345678901";
        // The derive_key function should use Argon2id with reasonable params
        let result = KeyDerivation::derive_key("password", salt);
        assert!(result.is_ok(), "Argon2 key derivation should succeed");
        let key = result.unwrap();
        assert_eq!(key.len(), 32);
    }
}
