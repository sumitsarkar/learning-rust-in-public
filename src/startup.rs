use crate::routes;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use sqlx::SqlitePool;
use std::env;
use std::io::Error;
use std::net::TcpListener;
use std::path::Path;
use tracing_actix_web::TracingLogger;

pub fn run(listener: TcpListener, db_pool: SqlitePool) -> Result<Server, Error> {
    let db_pool = web::Data::new(db_pool);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route(
                "/health_check",
                web::get().to(routes::health_check::health_check),
            )
            .route(
                "/subscriptions",
                web::post().to(routes::subscriptions::subscribe),
            )
            .app_data(db_pool.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}

pub async fn run_migration(db_pool: &SqlitePool) {
    let migrations = if env::var("APP_ENVIRONMENT") == Ok("production".to_string()) {
        // Productions migrations dir
        std::env::current_exe()
            .expect("Error getting executable path")
            .join("./migrations")
    } else {
        // Development migrations dir
        let crate_dir =
            std::env::var("CARGO_MANIFEST_DIR").expect("Error getting Crate Directory.");
        Path::new(&crate_dir).join("./migrations")
    };

    tracing::info!("Running migrations with path: {:?}", migrations);

    sqlx::migrate::Migrator::new(migrations)
        .await
        .expect("Failed to create migrator")
        .run(db_pool)
        .await
        .expect("Failed to migrate database");
}
