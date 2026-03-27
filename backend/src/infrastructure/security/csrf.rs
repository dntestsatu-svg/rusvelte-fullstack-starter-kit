use rand::{distributions::Alphanumeric, Rng};

pub fn generate_csrf_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

pub fn verify_csrf_token(token: &str, expected: &str) -> bool {
    // Basic string comparison. In future could be HMAC-based.
    !token.is_empty() && token == expected
}
