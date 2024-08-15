use crate::configuration::{get_environment, DatabaseSettings, Settings};
use crate::email_client::EmailClient;
use crate::routes;
use crate::routes::newsletters::publish_newsletter;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use secrecy::ExposeSecret;
use sqlx::SqlitePool;
use std::env;
use std::io::Error;
use std::net::TcpListener;
use std::path::Path;
use tracing_actix_web::TracingLogger;

pub struct Application {
    port: u16,
    server: Server,
}

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

impl Application {
    pub async fn build(configuration: Settings, pool: Option<SqlitePool>) -> Result<Self, Error> {
        let connection_pool = get_connection_pool(&configuration.database, pool).await;

        match get_environment() {
            crate::configuration::Environment::Local => {
                tracing::info!("Make sure you build the database and run migrations manually.")
            }
            crate::configuration::Environment::Production => {
                tracing::info!("Building database and running migrations...");
                configuration.database.create_database_if_missing().await;

                // Run Migrations
                run_migration(&connection_pool).await;
                tracing::info!("Migrations completed.");
            }
        }

        let sender_email = configuration
            .email_client
            .sender()
            .expect("Invalid sender email address.");
        let timeout = configuration.email_client.timeout();
        let email_client = EmailClient::new(
            configuration.email_client.base_url,
            sender_email,
            configuration.email_client.authorization_token,
            timeout,
        );

        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );

        dbg!(&address);
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            connection_pool,
            email_client,
            configuration.application.base_url,
        )?;
        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub struct ApplicationBaseUrl(pub String);

pub fn run(
    listener: TcpListener,
    db_pool: SqlitePool,
    email_client: EmailClient,
    base_url: String,
) -> Result<Server, Error> {
    let db_pool = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);
    let base_url = web::Data::new(ApplicationBaseUrl(base_url));
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
            .route(
                "/subscriptions/confirm",
                web::get().to(routes::subscriptions_confirm::confirm),
            )
            .route("/newsletters", web::post().to(publish_newsletter))
            .app_data(db_pool.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}

pub async fn run_migration(db_pool: &SqlitePool) {
    let migrations = if env::var("APP_ENVIRONMENT") == Ok("production".to_string()) {
        Path::new("/app/migrations").join("")
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
