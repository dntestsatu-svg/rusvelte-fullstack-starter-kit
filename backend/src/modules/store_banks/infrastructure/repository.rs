use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::infrastructure::db::DbPool;
use crate::modules::store_banks::domain::entity::{
    NewStoreBankAccountRecord, StoreBankAccountSecret, StoreBankAccountSummary,
    StoreBankStoreProfile, StoreBankVerificationStatus,
};
use crate::modules::store_banks::domain::repository::StoreBankRepository;

pub struct SqlxStoreBankRepository {
    db: DbPool,
    encryption_key: String,
}

impl SqlxStoreBankRepository {
    pub fn new(db: DbPool, encryption_key: String) -> Self {
        Self { db, encryption_key }
    }
}

#[async_trait]
impl StoreBankRepository for SqlxStoreBankRepository {
    async fn find_store_profile(
        &self,
        store_id: Uuid,
    ) -> anyhow::Result<Option<StoreBankStoreProfile>> {
        sqlx::query_as::<_, StoreProfileRow>(
            r#"
            SELECT id AS store_id, owner_user_id, name AS store_name
            FROM stores
            WHERE id = $1
              AND deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(store_id)
        .fetch_optional(&self.db)
        .await
        .map(|record| record.map(Into::into))
        .map_err(Into::into)
    }

    async fn list_by_store(&self, store_id: Uuid) -> anyhow::Result<Vec<StoreBankAccountSummary>> {
        sqlx::query_as::<_, StoreBankAccountSummaryRow>(
            r#"
            SELECT
                id,
                store_id,
                owner_user_id,
                bank_code,
                bank_name,
                account_holder_name,
                account_number_last4,
                is_default,
                verification_status,
                verified_at,
                created_at,
                updated_at
            FROM store_bank_accounts
            WHERE store_id = $1
            ORDER BY is_default DESC, created_at DESC
            "#,
        )
        .bind(store_id)
        .fetch_all(&self.db)
        .await
        .map(|rows| rows.into_iter().map(Into::into).collect())
        .map_err(Into::into)
    }

    async fn insert_verified_account(
        &self,
        record: NewStoreBankAccountRecord,
    ) -> anyhow::Result<StoreBankAccountSummary> {
        let mut transaction = self.db.begin().await?;

        let existing_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM store_bank_accounts WHERE store_id = $1",
        )
        .bind(record.store_id)
        .fetch_one(&mut *transaction)
        .await?;

        let should_be_default = record.is_default || existing_count == 0;

        if should_be_default {
            sqlx::query(
                r#"
                UPDATE store_bank_accounts
                SET is_default = FALSE,
                    updated_at = $2
                WHERE store_id = $1
                  AND is_default = TRUE
                "#,
            )
            .bind(record.store_id)
            .bind(record.updated_at)
            .execute(&mut *transaction)
            .await?;
        }

        let inserted = sqlx::query_as::<_, StoreBankAccountSummaryRow>(
            r#"
            INSERT INTO store_bank_accounts (
                id,
                store_id,
                owner_user_id,
                bank_code,
                bank_name,
                account_holder_name,
                account_number_encrypted,
                account_number_last4,
                is_default,
                verification_status,
                verified_at,
                created_at,
                updated_at
            )
            VALUES (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                encode(pgp_sym_encrypt($7, $8), 'base64'),
                $9,
                $10,
                $11,
                $12,
                $13,
                $14
            )
            RETURNING
                id,
                store_id,
                owner_user_id,
                bank_code,
                bank_name,
                account_holder_name,
                account_number_last4,
                is_default,
                verification_status,
                verified_at,
                created_at,
                updated_at
            "#,
        )
        .bind(record.id)
        .bind(record.store_id)
        .bind(record.owner_user_id)
        .bind(record.bank_code)
        .bind(record.bank_name)
        .bind(record.account_holder_name)
        .bind(record.account_number_plaintext)
        .bind(&self.encryption_key)
        .bind(record.account_number_last4)
        .bind(should_be_default)
        .bind(record.verification_status.as_str())
        .bind(record.verified_at)
        .bind(record.created_at)
        .bind(record.updated_at)
        .fetch_one(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(inserted.into())
    }

    async fn set_default_bank(
        &self,
        store_id: Uuid,
        bank_account_id: Uuid,
        updated_at: DateTime<Utc>,
    ) -> anyhow::Result<Option<StoreBankAccountSummary>> {
        let mut transaction = self.db.begin().await?;

        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM store_bank_accounts
                WHERE id = $1
                  AND store_id = $2
            )
            "#,
        )
        .bind(bank_account_id)
        .bind(store_id)
        .fetch_one(&mut *transaction)
        .await?;

        if !exists {
            transaction.rollback().await?;
            return Ok(None);
        }

        sqlx::query(
            r#"
            UPDATE store_bank_accounts
            SET is_default = FALSE,
                updated_at = $2
            WHERE store_id = $1
              AND is_default = TRUE
            "#,
        )
        .bind(store_id)
        .bind(updated_at)
        .execute(&mut *transaction)
        .await?;

        let updated = sqlx::query_as::<_, StoreBankAccountSummaryRow>(
            r#"
            UPDATE store_bank_accounts
            SET is_default = TRUE,
                updated_at = $3
            WHERE store_id = $1
              AND id = $2
            RETURNING
                id,
                store_id,
                owner_user_id,
                bank_code,
                bank_name,
                account_holder_name,
                account_number_last4,
                is_default,
                verification_status,
                verified_at,
                created_at,
                updated_at
            "#,
        )
        .bind(store_id)
        .bind(bank_account_id)
        .bind(updated_at)
        .fetch_one(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(Some(updated.into()))
    }

    async fn find_account_with_secret(
        &self,
        store_id: Uuid,
        bank_account_id: Uuid,
    ) -> anyhow::Result<Option<StoreBankAccountSecret>> {
        sqlx::query_as::<_, StoreBankAccountSecretRow>(
            r#"
            SELECT
                id,
                store_id,
                owner_user_id,
                bank_code,
                bank_name,
                account_holder_name,
                account_number_last4,
                is_default,
                verification_status,
                verified_at,
                created_at,
                updated_at,
                pgp_sym_decrypt(decode(account_number_encrypted, 'base64'), $3) AS account_number_plaintext
            FROM store_bank_accounts
            WHERE store_id = $1
              AND id = $2
            LIMIT 1
            "#,
        )
        .bind(store_id)
        .bind(bank_account_id)
        .bind(&self.encryption_key)
        .fetch_optional(&self.db)
        .await?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn find_default_account_with_secret(
        &self,
        store_id: Uuid,
    ) -> anyhow::Result<Option<StoreBankAccountSecret>> {
        sqlx::query_as::<_, StoreBankAccountSecretRow>(
            r#"
            SELECT
                id,
                store_id,
                owner_user_id,
                bank_code,
                bank_name,
                account_holder_name,
                account_number_last4,
                is_default,
                verification_status,
                verified_at,
                created_at,
                updated_at,
                pgp_sym_decrypt(decode(account_number_encrypted, 'base64'), $2) AS account_number_plaintext
            FROM store_bank_accounts
            WHERE store_id = $1
              AND is_default = TRUE
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
        )
        .bind(store_id)
        .bind(&self.encryption_key)
        .fetch_optional(&self.db)
        .await?
        .map(TryInto::try_into)
        .transpose()
    }
}

#[derive(Debug, FromRow)]
struct StoreProfileRow {
    store_id: Uuid,
    owner_user_id: Uuid,
    store_name: String,
}

#[derive(Debug, FromRow)]
struct StoreBankAccountSummaryRow {
    id: Uuid,
    store_id: Uuid,
    owner_user_id: Uuid,
    bank_code: String,
    bank_name: String,
    account_holder_name: String,
    account_number_last4: String,
    is_default: bool,
    verification_status: String,
    verified_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct StoreBankAccountSecretRow {
    id: Uuid,
    store_id: Uuid,
    owner_user_id: Uuid,
    bank_code: String,
    bank_name: String,
    account_holder_name: String,
    account_number_last4: String,
    is_default: bool,
    verification_status: String,
    verified_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    account_number_plaintext: String,
}

impl From<StoreProfileRow> for StoreBankStoreProfile {
    fn from(value: StoreProfileRow) -> Self {
        Self {
            store_id: value.store_id,
            owner_user_id: value.owner_user_id,
            store_name: value.store_name,
        }
    }
}

impl From<StoreBankAccountSummaryRow> for StoreBankAccountSummary {
    fn from(value: StoreBankAccountSummaryRow) -> Self {
        Self {
            id: value.id,
            store_id: value.store_id,
            owner_user_id: value.owner_user_id,
            bank_code: value.bank_code,
            bank_name: value.bank_name,
            account_holder_name: value.account_holder_name,
            account_number_last4: value.account_number_last4,
            is_default: value.is_default,
            verification_status: value.verification_status.into(),
            verified_at: value.verified_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl TryFrom<StoreBankAccountSecretRow> for StoreBankAccountSecret {
    type Error = anyhow::Error;

    fn try_from(value: StoreBankAccountSecretRow) -> Result<Self, Self::Error> {
        if value.account_number_plaintext.trim().is_empty() {
            return Err(anyhow!("bank account decryption returned an empty value"));
        }

        Ok(Self {
            account: StoreBankAccountSummary {
                id: value.id,
                store_id: value.store_id,
                owner_user_id: value.owner_user_id,
                bank_code: value.bank_code,
                bank_name: value.bank_name,
                account_holder_name: value.account_holder_name,
                account_number_last4: value.account_number_last4,
                is_default: value.is_default,
                verification_status: StoreBankVerificationStatus::from(value.verification_status),
                verified_at: value.verified_at,
                created_at: value.created_at,
                updated_at: value.updated_at,
            },
            account_number_plaintext: value.account_number_plaintext,
        })
    }
}
