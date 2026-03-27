use std::sync::Arc;
use backend::bootstrap::config::Config;
use backend::infrastructure::db::init_db_pool;
use backend::infrastructure::redis::init_redis_pool;
use backend::modules::auth::application::dto::LoginRequest;
use backend::modules::auth::application::service::AuthService;
use backend::modules::auth::infrastructure::persistence::PostgresAuthRepository;
use backend::infrastructure::security::captcha::NoOpCaptchaVerifier;
use backend::infrastructure::security::limiter::SlidingWindowLimiter;
use backend::modules::auth::domain::repository::AuthRepository;

// Strictly proof-only. No repair logic. No manual seeding.
fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        if let Err(e) = run_proof().await {
            eprintln!("Proof failed! ERROR: {}", e);
            std::process::exit(1);
        }
    });
}

async fn run_proof() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;
    
    let db: backend::infrastructure::db::DbPool = init_db_pool(&config.database_url).await?;
    let redis: backend::infrastructure::redis::RedisPool = init_redis_pool(&config.redis_url).await?;

    let repo = Arc::new(PostgresAuthRepository::new(db.clone()));
    let captcha = Arc::new(NoOpCaptchaVerifier);
    let service = AuthService::new(repo.clone(), captcha, redis.clone());

    println!("--- PROOF: Issue #4 Auth & Security (Clean Room) ---");

    // 1. Schema Check: csrf_token column
    let column_exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name='sessions' AND column_name='csrf_token')"
    ).fetch_one(&db).await?;
    if !column_exists.0 {
        anyhow::bail!("SCHEMA PROOF FAILED: column 'csrf_token' missing from 'sessions' table!");
    }
    println!("[PASS] Schema: 'csrf_token' column exists in 'sessions' table.");

    // 2. Seed Check: exactly 1 dev user
    let dev_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'dev'").fetch_one(&db).await?;
    if dev_count.0 != 1 {
        anyhow::bail!("SEED PROOF FAILED: expected exactly 1 dev user, found {}!", dev_count.0);
    }
    let dev_user = repo.find_user_by_email("dev@justqiu.com").await?
        .ok_or_else(|| anyhow::anyhow!("SEED PROOF FAILED: dev@justqiu.com not found!"))?;
    println!("[PASS] Seed: Exactly 1 dev user exists ({})", dev_user.email);

    // 3. Login Proof: Valid Credentials -> 200
    let login_req = LoginRequest {
        email: "dev@justqiu.com".to_string(),
        password: "dev12345".to_string(),
        captcha_token: "dev-pass".to_string(),
    };
    let login_res = service.login(login_req).await?;
    println!("[PASS] Login: Valid credentials returns 200 equivalent. User: {}", login_res.user.email);
    let session_id = login_res.session_id;

    // 4. Session Context Proof: GET /me equivalent
    let context = service.resolve_session(session_id).await?
        .ok_or_else(|| anyhow::anyhow!("SESSION PROOF FAILED: valid session not found in Cache/DB!"))?;
    println!("[PASS] Session: Resolved valid session for {}", context.user.email);

    // 5. Login Proof: Wrong Password -> 401
    let wrong_pw_req = LoginRequest {
        email: "dev@justqiu.com".to_string(),
        password: "wrong".to_string(),
        captcha_token: "dev-pass".to_string(),
    };
    match service.login(wrong_pw_req).await {
        Err(e) if e.to_string().contains("Invalid credentials") => println!("[PASS] Login: Wrong password returns 401 equivalent"),
        other => anyhow::bail!("LOGIN PROOF FAILED: expected 401 equivalent for wrong password, got {:?}", other),
    }

    // 6. Login Proof: Invalid Captcha -> 400
    let bad_captcha_req = LoginRequest {
        email: "dev@justqiu.com".to_string(),
        password: "dev12345".to_string(),
        captcha_token: "bad".to_string(),
    };
    match service.login(bad_captcha_req).await {
        Err(e) if e.to_string().contains("Invalid captcha") => println!("[PASS] Login: Invalid captcha returns 400 equivalent"),
        other => anyhow::bail!("LOGIN PROOF FAILED: expected 400 equivalent for invalid captcha, got {:?}", other),
    }

    // 7. CSRF Proof
    if login_res.csrf_token.is_empty() {
        anyhow::bail!("CSRF PROOF FAILED: CSRF token is empty!");
    }
    println!("[PASS] CSRF: Token generated successfully.");

    // 8. Logout Proof: Invalidation
    service.logout(session_id).await?;
    let resolved_after = service.resolve_session(session_id).await?;
    if resolved_after.is_some() {
        anyhow::bail!("LOGOUT PROOF FAILED: session still valid after logout!");
    }
    println!("[PASS] Logout: Session invalidated successfully.");

    // 9. Rate Limiter Proof
    let mut conn = redis.get().await?;
    let key = format!("proof_limiter_{}", uuid::Uuid::new_v4());
    for i in 1..=5 {
        let allowed = SlidingWindowLimiter::is_allowed(&mut *conn, &key, 3, 10).await?;
        if i > 3 && allowed {
            anyhow::bail!("LIMITER PROOF FAILED: allowed request {} after limit of 3!", i);
        }
    }
    println!("[PASS] Rate Limiter: Blocked after 3 requests.");

    println!("--- END PROOF: ALL PASSED (Clean Room) ---");
    Ok(())
}
