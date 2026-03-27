use redis::{AsyncCommands, RedisError};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SlidingWindowLimiter;

impl SlidingWindowLimiter {
    pub async fn is_allowed<C>(
        redis: &mut C,
        key: &str,
        limit: u64,
        window_seconds: u64,
    ) -> Result<bool, RedisError>
    where
        C: AsyncCommands + Send,
    {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let window_start = now.saturating_sub(window_seconds * 1000);

        let full_key = format!("limiter:{}", key);

        // Sliding window logic using Redis ZSET
        // 1. Remove old entries
        let _: () = redis.zrembyscore(&full_key, 0, window_start as f64).await?;

        // 2. Count current entries
        let count: u64 = redis.zcard(&full_key).await?;

        if count >= limit {
            return Ok(false);
        }

        // 3. Add current request with unique score (nanos or unique ID in value if needed, but millis is usually enough)
        // Use a unique value (current time as string or similar) to ensure multiple entries per milli if possible,
        // but for this proof, millis is fine.
        let val = format!("{}:{}", now, uuid::Uuid::new_v4());
        let _: () = redis.zadd(&full_key, val, now as f64).await?;

        // 4. Set expiry for the whole set to window_seconds to keep Redis clean
        let _: () = redis.expire(&full_key, window_seconds as i64).await?;

        Ok(true)
    }
}
