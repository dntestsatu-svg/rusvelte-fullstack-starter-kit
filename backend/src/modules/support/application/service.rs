use crate::infrastructure::security::captcha::CaptchaVerifier;
use crate::modules::support::domain::entity::{
    ContactMessage, ContactThread, SenderType, ThreadStatus,
};
use crate::modules::support::infrastructure::repository::SupportRepository;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SubmitContactRequest {
    pub name: String,
    pub email: String,
    pub subject: String,
    pub category: String,
    pub message: String,
    pub captcha_token: String,
}

#[derive(Debug, Deserialize)]
pub struct ReplyRequest {
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct StatusUpdateRequest {
    pub status: ThreadStatus,
}

#[derive(Debug, Serialize)]
pub struct ThreadDetailResponse {
    pub thread: ContactThread,
    pub messages: Vec<ContactMessage>,
}

pub struct SupportService {
    repository: SupportRepository,
    captcha_verifier: Arc<dyn CaptchaVerifier>,
}

impl SupportService {
    pub fn new(repository: SupportRepository, captcha_verifier: Arc<dyn CaptchaVerifier>) -> Self {
        Self {
            repository,
            captcha_verifier,
        }
    }

    pub async fn submit_contact(&self, req: SubmitContactRequest) -> Result<ContactThread> {
        // 1. Verify Captcha
        if !self.captcha_verifier.verify(&req.captcha_token).await {
            return Err(anyhow!("Invalid captcha"));
        }

        // 2. Validate basic input
        if req.name.is_empty()
            || req.email.is_empty()
            || req.subject.is_empty()
            || req.message.is_empty()
        {
            return Err(anyhow!("Missing required fields"));
        }

        // 3. Create thread
        self.repository
            .create_thread(req.name, req.email, req.subject, req.category, req.message)
            .await
    }

    pub async fn list_threads(&self, limit: i64, offset: i64) -> Result<Vec<ContactThread>> {
        self.repository.list_threads(limit, offset).await
    }

    pub async fn get_thread_detail(&self, id: Uuid) -> Result<Option<ThreadDetailResponse>> {
        let thread = self.repository.get_thread_by_id(id).await?;
        if let Some(t) = thread {
            let messages = self.repository.get_thread_messages(id).await?;
            Ok(Some(ThreadDetailResponse {
                thread: t,
                messages,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn reply_to_thread(
        &self,
        thread_id: Uuid,
        staff_user_id: Uuid,
        req: ReplyRequest,
    ) -> Result<ContactMessage> {
        // Ensure thread exists
        let thread = self.repository.get_thread_by_id(thread_id).await?;
        if thread.is_none() {
            return Err(anyhow!("Thread not found"));
        }

        self.repository
            .add_message(thread_id, SenderType::Staff, Some(staff_user_id), req.body)
            .await
    }

    pub async fn update_thread_status(&self, id: Uuid, req: StatusUpdateRequest) -> Result<()> {
        self.repository.update_status(id, req.status).await
    }
}
