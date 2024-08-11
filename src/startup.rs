use crate::routes;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use sqlx::SqlitePool;
use std::io::Error;
use std::net::TcpListener;
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
    sqlx::migrate!("./migrations").run(db_pool).await.expect("Failed to migrate database");
}
