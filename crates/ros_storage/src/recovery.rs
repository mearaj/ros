//! Portable recovery envelope (ADR 0005) helpers.
//!
//! The envelope wraps the SQLCipher database key with an Owner passphrase so a
//! clean installation can restore without the original OS keystore entry.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::password_hash::SaltString;
use argon2::{
    Algorithm, Argon2, Params as Argon2Params, PasswordHash, PasswordHasher, PasswordVerifier,
    Version,
};
use rand_core::{OsRng, RngCore};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::StorageError;

pub const RECOVERY_ENVELOPE_VERSION: &str = "ros.recovery.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortableRecoveryEnvelope {
    pub envelope_json: String,
    pub backup_sha256: String,
}

pub fn hash_recovery_passphrase(passphrase: &str) -> Result<String, StorageError> {
    validate_recovery_passphrase(passphrase)?;
    let secret = Zeroizing::new(passphrase.as_bytes().to_vec());
    let salt = SaltString::generate(&mut OsRng);
    recovery_argon2()?
        .hash_password(secret.as_slice(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| StorageError::InvalidPersistedData("recovery hash failed".to_owned()))
}

pub fn verify_recovery_passphrase(passphrase: &str, digest: &str) -> bool {
    let Ok(_) = validate_recovery_passphrase(passphrase) else {
        return false;
    };
    let secret = Zeroizing::new(passphrase.as_bytes().to_vec());
    let Ok(parsed) = PasswordHash::new(digest) else {
        return false;
    };
    recovery_argon2()
        .ok()
        .and_then(|argon2| argon2.verify_password(secret.as_slice(), &parsed).ok())
        .is_some()
}

pub fn wrap_database_key(
    database_key: &[u8; 32],
    passphrase: &str,
    installation_id: &str,
    schema_version: i64,
    backup_sha256: &str,
    created_at_utc: &str,
    created_by_actor_id: &str,
) -> Result<PortableRecoveryEnvelope, StorageError> {
    validate_recovery_passphrase(passphrase)?;
    let mut salt = [0_u8; 16];
    OsRng.fill_bytes(&mut salt);
    let wrapping_key = derive_wrapping_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(wrapping_key.as_slice())
        .map_err(|_| StorageError::InvalidPersistedData("cipher init failed".to_owned()))?;
    let mut nonce_bytes = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, database_key.as_ref())
        .map_err(|_| StorageError::InvalidPersistedData("key wrap failed".to_owned()))?;
    let envelope = json!({
        "schema_version": 1,
        "envelope_version": RECOVERY_ENVELOPE_VERSION,
        "installation_id": installation_id,
        "schema_migration_version": schema_version,
        "backup_sha256": backup_sha256,
        "created_at_utc": created_at_utc,
        "created_by_actor_id": created_by_actor_id,
        "kdf": {
            "alg": "argon2id",
            "salt_hex": hex_encode(&salt),
        },
        "wrap": {
            "alg": "aes-256-gcm",
            "nonce_hex": hex_encode(&nonce_bytes),
            "ciphertext_hex": hex_encode(&ciphertext),
        }
    });
    Ok(PortableRecoveryEnvelope {
        envelope_json: envelope.to_string(),
        backup_sha256: backup_sha256.to_owned(),
    })
}

pub fn unwrap_database_key(
    envelope_json: &str,
    passphrase: &str,
    expected_backup_sha256: &str,
) -> Result<[u8; 32], StorageError> {
    validate_recovery_passphrase(passphrase)?;
    let value: Value = serde_json::from_str(envelope_json)
        .map_err(|_| StorageError::InvalidPersistedData("invalid recovery envelope".to_owned()))?;
    if value.get("envelope_version").and_then(Value::as_str) != Some(RECOVERY_ENVELOPE_VERSION) {
        return Err(StorageError::InvalidPersistedData(
            "unsupported recovery envelope version".to_owned(),
        ));
    }
    let backup_sha256 = value
        .get("backup_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| StorageError::InvalidPersistedData("missing backup hash".to_owned()))?;
    if backup_sha256 != expected_backup_sha256 {
        return Err(StorageError::InvalidPersistedData(
            "recovery envelope does not match backup checksum".to_owned(),
        ));
    }
    let salt = hex_decode(
        value
            .pointer("/kdf/salt_hex")
            .and_then(Value::as_str)
            .ok_or_else(|| StorageError::InvalidPersistedData("missing salt".to_owned()))?,
    )?;
    let nonce = hex_decode(
        value
            .pointer("/wrap/nonce_hex")
            .and_then(Value::as_str)
            .ok_or_else(|| StorageError::InvalidPersistedData("missing nonce".to_owned()))?,
    )?;
    let ciphertext = hex_decode(
        value
            .pointer("/wrap/ciphertext_hex")
            .and_then(Value::as_str)
            .ok_or_else(|| StorageError::InvalidPersistedData("missing ciphertext".to_owned()))?,
    )?;
    if salt.len() != 16 || nonce.len() != 12 {
        return Err(StorageError::InvalidPersistedData(
            "invalid wrap parameters".to_owned(),
        ));
    }
    let mut salt_arr = [0_u8; 16];
    salt_arr.copy_from_slice(&salt);
    let wrapping_key = derive_wrapping_key(passphrase, &salt_arr)?;
    let cipher = Aes256Gcm::new_from_slice(wrapping_key.as_slice())
        .map_err(|_| StorageError::InvalidPersistedData("cipher init failed".to_owned()))?;
    let nonce = Nonce::from_slice(&nonce);
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| StorageError::InvalidPersistedData("recovery passphrase rejected".to_owned()))?;
    if plaintext.len() != 32 {
        return Err(StorageError::InvalidPersistedData(
            "unwrapped key length invalid".to_owned(),
        ));
    }
    let mut key = [0_u8; 32];
    key.copy_from_slice(&plaintext);
    Ok(key)
}

fn validate_recovery_passphrase(passphrase: &str) -> Result<(), StorageError> {
    let len = passphrase.chars().count();
    if (24..=64).contains(&len) {
        Ok(())
    } else {
        Err(StorageError::InvalidPersistedData(
            "recovery passphrase must be 24 to 64 characters".to_owned(),
        ))
    }
}

fn recovery_argon2() -> Result<Argon2<'static>, StorageError> {
    #[cfg(not(test))]
    let parameters = Argon2Params::new(19_456, 2, 1, None)
        .map_err(|_| StorageError::StaffCredentialUnavailable)?;
    #[cfg(test)]
    let parameters = Argon2Params::new(4_096, 1, 1, None)
        .map_err(|_| StorageError::StaffCredentialUnavailable)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, parameters))
}

fn derive_wrapping_key(passphrase: &str, salt: &[u8; 16]) -> Result<Zeroizing<[u8; 32]>, StorageError> {
    let mut key = Zeroizing::new([0_u8; 32]);
    recovery_argon2()?
        .hash_password_into(passphrase.as_bytes(), salt, key.as_mut())
        .map_err(|_| StorageError::InvalidPersistedData("key derivation failed".to_owned()))?;
    // Mix in a domain separator so PIN hashes and wrap keys never collide.
    let mut hasher = Sha256::new();
    hasher.update(b"ros.recovery.wrap.v1");
    hasher.update(key.as_slice());
    let digest = hasher.finalize();
    key.copy_from_slice(&digest);
    Ok(key)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(value: &str) -> Result<Vec<u8>, StorageError> {
    if !value.len().is_multiple_of(2) {
        return Err(StorageError::InvalidPersistedData(
            "invalid hex encoding".to_owned(),
        ));
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16).map_err(|_| {
                StorageError::InvalidPersistedData("invalid hex encoding".to_owned())
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_and_unwrap_round_trip() {
        let key = [7_u8; 32];
        let passphrase = "owner-recovery-passphrase-ok";
        let envelope = wrap_database_key(
            &key,
            passphrase,
            "install-1",
            34,
            "sha256:abc",
            "2026-07-18T00:00:00.000Z",
            "actor-1",
        )
        .expect("wrap");
        let opened = unwrap_database_key(&envelope.envelope_json, passphrase, "sha256:abc")
            .expect("unwrap");
        assert_eq!(opened, key);
        assert!(
            unwrap_database_key(&envelope.envelope_json, "wrong-recovery-passphrase!!", "sha256:abc")
                .is_err()
        );
    }
}
