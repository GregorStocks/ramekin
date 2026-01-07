use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2, Params,
};
use rand::rngs::OsRng;
use rand::TryRngCore;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

/// Use insecure (fast) password hashing for dev/test environments
static INSECURE_HASHING: LazyLock<bool> =
    LazyLock::new(|| std::env::var("INSECURE_PASSWORD_HASHING").is_ok());

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .expect("Failed to generate random bytes");
    hex::encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn get_argon2() -> Argon2<'static> {
    if *INSECURE_HASHING {
        // Minimal params for fast dev/test - NOT SECURE FOR PRODUCTION
        let params = Params::new(1024, 1, 1, None).unwrap();
        Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
    } else {
        Argon2::default()
    }
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let argon2 = get_argon2();
    let hash = argon2.hash_password(password.as_bytes())?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    get_argon2()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}
