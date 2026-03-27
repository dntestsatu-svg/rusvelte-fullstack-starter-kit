use crate::modules::support::domain::entity::{
    ContactMessage, ContactThread, SenderType, ThreadStatus,
};
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct SupportRepository {
    pool: PgPool,
}

impl SupportRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_thread(
        &self,
        name: String,
        email: String,
        subject: String,
        category: String,
        initial_message: String,
    ) -> Result<ContactThread> {
        let mut tx = self.pool.begin().await?;

        let thread_id = Uuid::new_v4();
        let thread = sqlx::query_as!(
            ContactThread,
            r#"
            INSERT INTO contact_threads (id, name, email, subject, category, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, now(), now())
            RETURNING id, name, email, phone, company_name, subject, category, status as "status: ThreadStatus", assigned_to_user_id, last_message_at, created_at, updated_at
            "#,
            thread_id,
            name,
            email,
            subject,
            category,
            ThreadStatus::Open.to_string()
        )
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query!(
            r#"
            INSERT INTO contact_thread_messages (id, thread_id, sender_type, body, created_at)
            VALUES ($1, $2, $3, $4, now())
            "#,
            Uuid::new_v4(),
            thread.id,
            SenderType::Guest.to_string(),
            initial_message
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(thread)
    }

    pub async fn list_threads(&self, limit: i64, offset: i64) -> Result<Vec<ContactThread>> {
        let threads = sqlx::query_as!(
            ContactThread,
            r#"
            SELECT id, name, email, phone, company_name, subject, category, status as "status: ThreadStatus", assigned_to_user_id, last_message_at, created_at, updated_at
            FROM contact_threads
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(threads)
    }

    pub async fn get_thread_by_id(&self, id: Uuid) -> Result<Option<ContactThread>> {
        let thread = sqlx::query_as!(
            ContactThread,
            r#"
            SELECT id, name, email, phone, company_name, subject, category, status as "status: ThreadStatus", assigned_to_user_id, last_message_at, created_at, updated_at
            FROM contact_threads
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(thread)
    }

    pub async fn get_thread_messages(&self, thread_id: Uuid) -> Result<Vec<ContactMessage>> {
        let messages = sqlx::query_as!(
            ContactMessage,
            r#"
            SELECT id, thread_id, sender_type as "sender_type: SenderType", sender_user_id, body, created_at
            FROM contact_thread_messages
            WHERE thread_id = $1
            ORDER BY created_at ASC
            "#,
            thread_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(messages)
    }

    pub async fn add_message(
        &self,
        thread_id: Uuid,
        sender_type: SenderType,
        sender_user_id: Option<Uuid>,
        body: String,
    ) -> Result<ContactMessage> {
        let mut tx = self.pool.begin().await?;

        let message = sqlx::query_as!(
            ContactMessage,
            r#"
            INSERT INTO contact_thread_messages (id, thread_id, sender_type, sender_user_id, body, created_at)
            VALUES ($1, $2, $3, $4, $5, now())
            RETURNING id, thread_id, sender_type as "sender_type: SenderType", sender_user_id, body, created_at
            "#,
            Uuid::new_v4(),
            thread_id,
            sender_type.to_string(),
            sender_user_id,
            body
        )
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query!(
            r#"
            UPDATE contact_threads
            SET updated_at = now(), last_message_at = now()
            WHERE id = $1
            "#,
            thread_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(message)
    }

    pub async fn update_status(&self, id: Uuid, status: ThreadStatus) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE contact_threads
            SET status = $1, updated_at = now()
            WHERE id = $2
            "#,
            status.to_string(),
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
