use redis::{AsyncCommands, RedisError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IdempotencyStatus {
    Pending,
    Completed(String),
}

pub async fn get_idempotency<C>(
    redis: &mut C,
    key: &str,
) -> Result<Option<IdempotencyStatus>, RedisError>
where
    C: AsyncCommands + Send,
{
    let full_key = format!("idempotency:{}", key);
    let val: Option<String> = redis.get(&full_key).await?;

    match val {
        Some(s) if s == "PENDING" => Ok(Some(IdempotencyStatus::Pending)),
        Some(s) => Ok(Some(IdempotencyStatus::Completed(s))),
        None => Ok(None),
    }
}

pub async fn save_idempotency_pending<C>(
    redis: &mut C,
    key: &str,
    ttl_secs: u64,
) -> Result<(), RedisError>
where
    C: AsyncCommands + Send,
{
    let full_key = format!("idempotency:{}", key);
    redis.set_ex(full_key, "PENDING", ttl_secs).await
}

pub async fn save_idempotency_completed<C>(
    redis: &mut C,
    key: &str,
    response: &str,
    ttl_secs: u64,
) -> Result<(), RedisError>
where
    C: AsyncCommands + Send,
{
    let full_key = format!("idempotency:{}", key);
    redis.set_ex(full_key, response, ttl_secs).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idempotency_status_serialization() {
        let s = IdempotencyStatus::Completed("{\"ok\":true}".to_string());
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("Completed"));
    }
}
