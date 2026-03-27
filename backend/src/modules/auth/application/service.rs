use super::dto::{LoginRequest, LoginResponse, SessionContext, UserProfile};
use crate::infrastructure::security::argon2::verify_password;
use crate::infrastructure::security::captcha::CaptchaVerifier;
use crate::infrastructure::security::csrf::generate_csrf_token;
use crate::modules::auth::domain::repository::AuthRepository;
use crate::modules::auth::domain::session::Session;
use chrono::{Duration, Utc};
use redis::AsyncCommands;
use std::sync::Arc;
use uuid::Uuid;

pub struct AuthService {
    repo: Arc<dyn AuthRepository>,
    captcha: Arc<dyn CaptchaVerifier>,
    redis_pool: bb8::Pool<bb8_redis::RedisConnectionManager>,
}

impl AuthService {
    pub fn new(
        repo: Arc<dyn AuthRepository>,
        captcha: Arc<dyn CaptchaVerifier>,
        redis_pool: bb8::Pool<bb8_redis::RedisConnectionManager>,
    ) -> Self {
        Self {
            repo,
            captcha,
            redis_pool,
        }
    }

    pub async fn get_redis_conn(
        &self,
    ) -> anyhow::Result<bb8::PooledConnection<'_, bb8_redis::RedisConnectionManager>> {
        let conn = self.redis_pool.get().await?;
        Ok(conn)
    }

    pub async fn login(&self, req: LoginRequest) -> anyhow::Result<LoginResponse> {
        // 1. Verify captcha
        if !self.captcha.verify(&req.captcha_token).await {
            return Err(anyhow::anyhow!("Invalid captcha"));
        }

        // 2. Find user
        let user = self
            .repo
            .find_user_by_email(&req.email)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Invalid credentials"))?;

        // Only active users are allowed to establish a session.
        if user.status != "active" {
            return Err(anyhow::anyhow!("Account is not active"));
        }

        // 3. Verify password
        if !verify_password(&req.password, &user.password_hash) {
            return Err(anyhow::anyhow!("Invalid credentials")); // Generic message for security
        }

        // 4. Create session
        let session_id = Uuid::new_v4();
        let csrf_token = generate_csrf_token();
        let expires_at = Utc::now() + Duration::hours(24);

        let session = Session {
            id: session_id,
            user_id: user.id,
            csrf_token: csrf_token.clone(),
            expires_at,
            created_at: Utc::now(),
        };

        self.repo.create_session(&session).await?;
        self.repo.update_last_login(user.id).await?;

        // 5. Build context and cache in Redis
        let profile = UserProfile {
            id: user.id,
            name: user.name,
            email: user.email,
            role: user.role,
            status: user.status,
        };

        let context = SessionContext {
            session_id,
            user: profile.clone(),
            csrf_token: csrf_token.clone(),
        };

        self.cache_session(&context, expires_at).await?;

        Ok(LoginResponse {
            user: profile,
            csrf_token,
            session_id,
        })
    }

    pub async fn logout(&self, session_id: Uuid) -> anyhow::Result<()> {
        self.repo.delete_session(session_id).await?;
        self.evict_session(session_id).await?;
        Ok(())
    }

    pub async fn resolve_session(
        &self,
        session_id: Uuid,
    ) -> anyhow::Result<Option<SessionContext>> {
        // 1. Try Redis
        if let Some(ctx) = self.get_cached_session(session_id).await? {
            return Ok(Some(ctx));
        }

        // 2. Fallback to DB
        let session = self.repo.find_session_by_id(session_id).await?;
        if let Some(s) = session {
            if s.is_expired() {
                let _ = self.repo.delete_session(session_id).await;
                let _ = self.evict_session(session_id).await;
                return Ok(None);
            }

            let user = self.repo.find_user_by_id(s.user_id).await?;
            if let Some(u) = user {
                if u.status != "active" {
                    let _ = self.repo.delete_session(session_id).await;
                    let _ = self.evict_session(session_id).await;
                    return Ok(None);
                }

                let ctx = SessionContext {
                    session_id: s.id,
                    user: UserProfile {
                        id: u.id,
                        name: u.name,
                        email: u.email,
                        role: u.role,
                        status: u.status,
                    },
                    csrf_token: s.csrf_token,
                };
                // 3. Rehydrate Redis
                self.cache_session(&ctx, s.expires_at).await?;
                return Ok(Some(ctx));
            }
        }

        Ok(None)
    }

    async fn cache_session(
        &self,
        ctx: &SessionContext,
        expires_at: chrono::DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let mut conn = self.redis_pool.get().await?;
        let key = format!("session:{}", ctx.session_id);
        let val = serde_json::to_string(ctx)?;
        let ttl = (expires_at - Utc::now()).num_seconds().max(0) as u64;

        let _: () = conn.set_ex(key, val, ttl).await?;
        Ok(())
    }

    async fn get_cached_session(&self, session_id: Uuid) -> anyhow::Result<Option<SessionContext>> {
        let mut conn = self.redis_pool.get().await?;
        let key = format!("session:{}", session_id);
        let val: Option<String> = conn.get(key).await?;

        match val {
            Some(s) => Ok(Some(serde_json::from_str(&s)?)),
            None => Ok(None),
        }
    }

    async fn evict_session(&self, session_id: Uuid) -> anyhow::Result<()> {
        let mut conn = self.redis_pool.get().await?;
        let key = format!("session:{}", session_id);
        let _: () = conn.del(key).await?;
        Ok(())
    }
}
