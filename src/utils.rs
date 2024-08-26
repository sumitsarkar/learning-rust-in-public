use actix_web::{http::header::LOCATION, HttpResponse};
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

///
/// Return an opaque 500 while preserving the error's root cause for logging.
pub fn e500<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static,
{
    actix_web::error::ErrorInternalServerError(e)
}

pub fn see_other(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((LOCATION, location))
        .finish()
}
