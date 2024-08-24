use std::{collections::HashMap, sync::Arc};

use actix_session::storage::{LoadError, SaveError, SessionKey, SessionStore, UpdateError};
use chrono::Utc;
use rand::distributions::{Alphanumeric, DistString};
use sqlx::SqlitePool;

#[derive(Clone)]
struct CacheConfiguration {
    cache_keygen: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

#[derive(Clone)]
pub struct SqlxSqliteSessionStore {
    configuration: CacheConfiguration,
    pool: SqlitePool,
}

impl Default for CacheConfiguration {
    fn default() -> Self {
        Self {
            cache_keygen: Arc::new(str::to_owned),
        }
    }
}

pub enum ConnectionData {
    AbsolutePathToDb(String),
    ConnectionPool(SqlitePool),
}

#[must_use]
pub struct SqlxSqliteSessionStoreBuilder {
    configuration: CacheConfiguration,
    pool: SqlitePool,
}

impl SqlxSqliteSessionStoreBuilder {
    pub fn build(self) -> SqlxSqliteSessionStore {
        SqlxSqliteSessionStore {
            pool: self.pool,
            configuration: self.configuration,
        }
    }

    /// Set a custom cache key generation strategy, expecting a session key as input.
    pub fn cache_keygen<F>(mut self, keygen: F) -> Self
    where
        F: Fn(&str) -> String + 'static + Send + Sync,
    {
        self.configuration.cache_keygen = Arc::new(keygen);
        self
    }
}

impl SqlxSqliteSessionStore {
    /// Returns a fluent API builder to configure [`SqlxSqliteSessionStore`].
    ///
    /// It takes as input the only required input to create a new instance of [`SqlxSqliteSessionStore`]
    /// - a pool object for Sqlite.
    pub fn builder_pooled(pool: impl Into<SqlitePool>) -> SqlxSqliteSessionStoreBuilder {
        SqlxSqliteSessionStoreBuilder {
            configuration: CacheConfiguration::default(),
            pool: pool.into(),
        }
    }

    /// Creates a new instance of [`SqlxSqliteSessionStore`] using the default configuration.
    ///
    /// It takes as input the only required input to create a new instance of [`SqlxSqliteSessionStore`]
    /// - a pool object for Sqlite.
    pub fn new_pooled(pool: impl Into<SqlitePool>) -> SqlxSqliteSessionStore {
        Self::builder_pooled(pool).build()
    }

    pub async fn cleanup(&self) -> Result<(), anyhow::Error> {
        sqlx::query!(r#"DELETE FROM sessions WHERE expires > unixepoch()"#)
            .execute(&self.pool)
            .await
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;
        Ok(())
    }
}

pub fn generate_session_key() -> SessionKey {
    Alphanumeric
        .sample_string(&mut rand::thread_rng(), 64)
        .try_into()
        .expect("generated string should be within size range for a session key")
}

pub(crate) type SessionState = HashMap<String, String>;

impl SessionStore for SqlxSqliteSessionStore {
    async fn load(
        &self,
        session_key: &actix_session::storage::SessionKey,
    ) -> Result<Option<SessionState>, actix_session::storage::LoadError> {
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());

        let row = sqlx::query!(
            "SELECT session FROM sessions where id = $1 AND expires > unixepoch()",
            cache_key
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
        .map_err(LoadError::Other)?;

        match row {
            None => Ok(None),
            Some(r) => {
                let state: SessionState = serde_json::from_str(&r.session)
                    .map_err(Into::into)
                    .map_err(LoadError::Deserialization)?;
                Ok(Some(state))
            }
        }
    }

    async fn save(
        &self,
        session_state: SessionState,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<actix_session::storage::SessionKey, actix_session::storage::SaveError> {
        let body = serde_json::to_string(&session_state)
            .map_err(Into::into)
            .map_err(SaveError::Serialization)?;
        let key = generate_session_key();
        let expires = Utc::now() + chrono::Duration::seconds(ttl.whole_seconds());
        let cache_key = (self.configuration.cache_keygen)(key.as_ref());

        sqlx::query!(r#"INSERT INTO sessions(id, session, expires) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"#, cache_key, body, expires)
        .execute(&self.pool)
        .await
        .map_err(Into::into)
        .map_err(SaveError::Other)?;
        Ok(key)
    }

    async fn update(
        &self,
        session_key: actix_session::storage::SessionKey,
        session_state: SessionState,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<actix_session::storage::SessionKey, actix_session::storage::UpdateError> {
        let body = serde_json::to_string(&session_state)
            .map_err(Into::into)
            .map_err(UpdateError::Serialization)?;
        let cache_key = (self.configuration.cache_keygen)(session_key.as_ref());
        let new_expires = Utc::now() + chrono::Duration::seconds(ttl.whole_seconds());

        sqlx::query!(
            r#"UPDATE sessions SET session = $1, expires = $2 WHERE id = $3"#,
            body,
            new_expires,
            cache_key
        )
        .execute(&self.pool)
        .await
        .map_err(Into::into)
        .map_err(UpdateError::Other)?;

        Ok(session_key)
    }

    async fn update_ttl(
        &self,
        session_key: &actix_session::storage::SessionKey,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<(), anyhow::Error> {
        let new_expires = Utc::now() + chrono::Duration::seconds(ttl.whole_seconds());
        let key = (self.configuration.cache_keygen)(session_key.as_ref());
        sqlx::query!(
            r#"UPDATE sessions SET expires = $1 WHERE id = $2"#,
            new_expires,
            key
        )
        .execute(&self.pool)
        .await
        .map_err(Into::into)
        .map_err(UpdateError::Other)?;

        Ok(())
    }

    async fn delete(
        &self,
        session_key: &actix_session::storage::SessionKey,
    ) -> Result<(), anyhow::Error> {
        let key = (self.configuration.cache_keygen)(session_key.as_ref());
        sqlx::query!(r#"DELETE FROM sessions WHERE id = $1"#, key)
            .execute(&self.pool)
            .await
            .map_err(Into::into)
            .map_err(UpdateError::Other)?;
        Ok(())
    }
}
