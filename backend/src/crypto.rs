use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2,
};

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let hash = Argon2::default().hash_password(password.as_bytes())?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(p) => p,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn new_session_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_from_argon2_0_5_still_verify() {
        let old = "$argon2id$v=19$m=19456,t=2,p=1$lAfiAAuaPKDvb6jbr+fLEQ$Tn1nuPhw/fL/sNF4kShO+FfUUJTx32nX8t2ovudpIUw";
        assert!(verify_password("hunter2", old));
        assert!(!verify_password("hunter3", old));
    }

    #[test]
    fn new_hashes_round_trip() {
        let h = hash_password("hunter2").unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(verify_password("hunter2", &h));
        assert!(!verify_password("nope", &h));
    }
}
