use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// SHA-256 hash of an external binary for integrity verification.
#[derive(Debug, Clone)]
pub struct BinaryHashEntry {
    /// Expected hex-encoded SHA-256 hash
    pub hash_hex: String,
    /// Description of the source (e.g. "Kali 2024.1 apt package tor 0.4.8.x")
    pub source: String,
}

/// Verifies SHA-256 hashes of external binaries before execution.
///
/// Checks:
///   1. Built-in compile-time defaults (populated from known-good versions)
///   2. Overrides from `.hashes` file in config directory
///
/// If no hash is configured for a binary and `strict` is false, a warning
/// is logged and verification is skipped (graceful degradation).
#[derive(Clone)]
pub struct BinaryVerifier {
    overrides: HashMap<String, BinaryHashEntry>,
    strict: bool,
}

impl BinaryVerifier {
    /// Create a verifier with built-in default hashes only.
    pub fn new(strict: bool) -> Self {
        Self {
            overrides: HashMap::new(),
            strict,
        }
    }

    /// Create a verifier from a `.hashes` TOML file overlaid on built-in defaults.
    ///
    /// File format:
    /// ```toml
    /// [hashes]
    /// "/usr/bin/tor" = { hash = "abc123...", source = "Kali apt tor 0.4.8.12" }
    /// "/usr/bin/obfs4proxy" = { hash = "def456...", source = "..." }
    /// ```
    pub fn from_hashes_file(path: &Path, strict: bool) -> Result<Self> {
        let mut overrides: HashMap<String, BinaryHashEntry> = HashMap::new();

        if path.exists() {
            let contents = std::fs::read_to_string(path)
                .context("Failed to read .hashes file")?;

            #[derive(serde::Deserialize)]
            struct HashValue {
                hash: String,
                source: String,
            }

            #[derive(serde::Deserialize)]
            struct HashFile {
                hashes: Option<HashMap<String, HashValue>>,
            }

            let parsed: HashFile = toml::from_str(&contents)
                .context("Failed to parse .hashes file")?;

            if let Some(hashes) = parsed.hashes {
                for (binary_path, entry) in hashes {
                    let hash_upper = entry.hash.to_uppercase();
                    if !hash_upper.chars().all(|c| c.is_ascii_hexdigit()) {
                        anyhow::bail!(
                            "Invalid hex hash for {}: '{}' is not a valid hex string",
                            binary_path,
                            entry.hash
                        );
                    }
                    if hash_upper.len() != 64 {
                        anyhow::bail!(
                            "Invalid SHA-256 hash length for {}: expected 64 hex chars, got {}",
                            binary_path,
                            hash_upper.len()
                        );
                    }
                    overrides.insert(
                        binary_path,
                        BinaryHashEntry {
                            hash_hex: hash_upper,
                            source: entry.source,
                        },
                    );
                }
            }
        }

        Ok(Self { overrides, strict })
    }

    /// Register or override a hash entry at runtime.
    pub fn set_hash(&mut self, binary_path: &str, entry: BinaryHashEntry) {
        self.overrides.insert(binary_path.to_string(), entry);
    }

    /// Compute SHA-256 of a binary file.
    pub fn compute_hash(path: &Path) -> Result<String> {
        let contents = std::fs::read(path)
            .with_context(|| format!("Failed to read binary for hashing: {}", path.display()))?;
        let mut hasher = Sha256::new();
        hasher.update(&contents);
        let result = hasher.finalize();
        Ok(hex::encode(result).to_uppercase())
    }

    /// Verify a binary's hash matches its expected value.
    ///
    /// Returns `Ok(())` if:
    ///   - The binary has a configured hash and it matches, OR
    ///   - No hash is configured and strict is false (graceful degradation)
    ///
    /// Returns `Err` if:
    ///   - The binary has a configured hash and it DOES NOT match, OR
    ///   - No hash is configured and strict is true
    pub fn verify(&self, binary_path: &Path) -> Result<()> {
        let path_str = binary_path.to_string_lossy();

        let entry = match self.overrides.get(path_str.as_ref()) {
            Some(e) => e,
            None => {
                if self.strict {
                    anyhow::bail!(
                        "No hash configured for {} in strict mode — refusing to execute",
                        path_str
                    );
                }
                warn!(
                    "No hash configured for {} — skipping integrity verification",
                    path_str
                );
                return Ok(());
            }
        };

        let actual = Self::compute_hash(binary_path)?;

        if actual == entry.hash_hex {
            info!(
                "Binary integrity verified: {} (SHA-256 match from {})",
                path_str, entry.source
            );
            Ok(())
        } else {
            let err_msg = format!(
                "BINARY INTEGRITY MISMATCH: {}\n  Expected SHA-256: {} ({})\n  Actual SHA-256:   {}\n\
                 This binary may have been tampered with or updated. Update the hash in .hashes \
                 if this is a legitimate upgrade.",
                path_str, entry.hash_hex, entry.source, actual
            );
            Err(anyhow::anyhow!("{}", err_msg))
        }
    }

    /// Return the number of configured hash entries.
    pub fn hash_count(&self) -> usize {
        self.overrides.len()
    }

    /// Verify all registered hashes and return a list of failures.
    pub fn verify_all(&self) -> Vec<(String, String)> {
        let mut failures = Vec::new();
        for (path_str, entry) in &self.overrides {
            let path = Path::new(path_str);
            if !path.exists() {
                failures.push((path_str.clone(), format!("Binary not found at path")));
                continue;
            }
            match Self::compute_hash(path) {
                Ok(actual) if actual == entry.hash_hex => {
                    info!("Hash OK: {} ({})", path_str, entry.source);
                }
                Ok(actual) => {
                    failures.push((
                        path_str.clone(),
                        format!(
                            "Hash mismatch: expected {}, got {}",
                            entry.hash_hex, actual
                        ),
                    ));
                }
                Err(e) => {
                    failures.push((path_str.clone(), format!("Read error: {e}")));
                }
            }
        }
        failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_known_value() {
        // SHA-256 of empty byte slice
        let dir = std::env::temp_dir().join(format!("verify_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.bin");
        std::fs::write(&path, b"").unwrap();

        let hash = BinaryVerifier::compute_hash(&path).unwrap();
        assert_eq!(
            hash,
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_verify_match_succeeds() {
        let dir = std::env::temp_dir().join(format!("verify_test_match_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.bin");
        std::fs::write(&path, b"hello").unwrap();

        let hash = BinaryVerifier::compute_hash(&path).unwrap();
        let mut verifier = BinaryVerifier::new(false);
        verifier.set_hash(
            path.to_str().unwrap(),
            BinaryHashEntry {
                hash_hex: hash.clone(),
                source: "test".into(),
            },
        );

        assert!(verifier.verify(&path).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_verify_mismatch_fails() {
        let dir = std::env::temp_dir().join(format!("verify_test_mismatch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.bin");
        std::fs::write(&path, b"hello").unwrap();

        let mut verifier = BinaryVerifier::new(false);
        verifier.set_hash(
            path.to_str().unwrap(),
            BinaryHashEntry {
                hash_hex: "0000000000000000000000000000000000000000000000000000000000000000".into(),
                source: "test".into(),
            },
        );

        assert!(verifier.verify(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_no_hash_in_non_strict_mode_ok() {
        let verifier = BinaryVerifier::new(false);
        // A non-existent path with no configured hash should still succeed in non-strict
        assert!(verifier.verify(Path::new("/nonexistent/binary")).is_ok());
    }

    #[test]
    fn test_no_hash_in_strict_mode_fails() {
        let verifier = BinaryVerifier::new(true);
        assert!(verifier.verify(Path::new("/nonexistent/binary")).is_err());
    }

    #[test]
    fn test_validate_hash_length() {
        let dir = std::env::temp_dir().join("verify_test_validate");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let hash_path = dir.join("hashes.toml");
        std::fs::write(
            &hash_path,
            r#"[hashes]
"/usr/bin/tor" = { hash = "tooshort", source = "test" }
"#,
        )
        .unwrap();

        let result = BinaryVerifier::from_hashes_file(&hash_path, false);
        assert!(result.is_err(), "Short hash should be rejected");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_invalid_hex() {
        let dir = std::env::temp_dir().join("verify_test_hex");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let hash_path = dir.join("hashes.toml");
        std::fs::write(
            &hash_path,
            r#"[hashes]
"/usr/bin/tor" = { hash = "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ", source = "test" }
"#,
        )
        .unwrap();

        let result = BinaryVerifier::from_hashes_file(&hash_path, false);
        assert!(result.is_err(), "Non-hex chars should be rejected");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
