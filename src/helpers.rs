use secrecy::ExposeSecret;
use sqlx::SqlitePool;

use crate::configuration::DatabaseSettings;

pub async fn get_connection_pool(
    database_configuration: &DatabaseSettings,
    pool: Option<SqlitePool>,
) -> SqlitePool {
    match pool {
        Some(p) => p,
        None => {
            let connection_pool = SqlitePool::connect_lazy(
                database_configuration.connection_string().expose_secret(),
            )
            .expect("Failed to connect to Sqlite.");
            connection_pool
        }
    }
}
