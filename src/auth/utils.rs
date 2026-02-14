use argon2::{PasswordHash, PasswordVerifier, Argon2};
use bcrypt::verify;
use scrypt::Scrypt;

pub fn verify_argon2(password: &str, hash: &str) -> bool {
    if let Ok(parsed_hash) = PasswordHash::new(hash) {
        Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok()
    } else {
        false
    }
}

pub fn verify_bcrypt(password: &str, hash: &str) -> bool {
    verify(password, hash).unwrap_or(false)
}

pub fn verify_scrypt(password: &str, hash: &str) -> bool {
    if let Ok(parsed_hash) = PasswordHash::new(hash) {
        Scrypt.verify_password(password.as_bytes(), &parsed_hash).is_ok()
    } else {
        false
    }
}
