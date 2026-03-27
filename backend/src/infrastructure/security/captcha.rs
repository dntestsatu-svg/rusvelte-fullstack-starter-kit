use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaResponse {
    pub success: bool,
    pub challenge_ts: Option<String>,
    pub hostname: Option<String>,
    pub error_codes: Option<Vec<String>>,
}

#[async_trait]
pub trait CaptchaVerifier: Send + Sync {
    async fn verify(&self, token: &str) -> bool;
}

pub struct NoOpCaptchaVerifier;

#[async_trait]
impl CaptchaVerifier for NoOpCaptchaVerifier {
    async fn verify(&self, _token: &str) -> bool {
        // For development/testing, we can use a "PASS" token
        _token == "dev-pass"
    }
}

pub struct HttpCaptchaVerifier {
    pub secret: String,
    pub endpoint: String,
}

#[async_trait]
impl CaptchaVerifier for HttpCaptchaVerifier {
    async fn verify(&self, token: &str) -> bool {
        let client = reqwest::Client::new();
        let res = client
            .post(&self.endpoint)
            .form(&[("secret", self.secret.as_str()), ("response", token)])
            .send()
            .await;

        match res {
            Ok(resp) => {
                let captcha_res: Result<CaptchaResponse, _> = resp.json().await;
                captcha_res.map(|r| r.success).unwrap_or(false)
            }
            Err(_) => false,
        }
    }
}
