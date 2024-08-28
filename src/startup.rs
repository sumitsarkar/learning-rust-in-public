use crate::authentication::middleware::reject_anonymous_users;
use crate::configuration::{get_environment, Settings};
use crate::email_client::{EmailClient};
use crate::routes::{self, site};
use crate::session::SqlxSqliteSessionStore;
use crate::utils::get_connection_pool;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::dev::Server;
use actix_web::middleware::from_fn;
use actix_web::rt::time;
use actix_web::{web, App, HttpServer};
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web_flash_messages::{FlashMessagesFramework, Level};
use secrecy::{ExposeSecret, Secret};
use sqlx::SqlitePool;
use std::env;
use std::io::Error;
use std::net::TcpListener;
use std::path::Path;
use std::time::Duration;
use tracing_actix_web::TracingLogger;

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(configuration: Settings, pool: Option<SqlitePool>) -> Result<Self, Error> {
        let connection_pool = get_connection_pool(&configuration.database, pool).await;
        let email_client = configuration.email_client.client();
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

        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );

        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            connection_pool,
            email_client,
            configuration.application.base_url,
            configuration.application.hmac_secret,
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
    hmac_secret: Secret<String>,
) -> Result<Server, Error> {
    let session_store = SqlxSqliteSessionStore::new_pooled(db_pool.clone());

    let db_pool_web = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);
    let base_url = web::Data::new(ApplicationBaseUrl(base_url));
    let secret_key: Key = Key::from(hmac_secret.expose_secret().as_bytes());
    let message_store = CookieMessageStore::builder(secret_key.clone()).build();
    let message_framework = FlashMessagesFramework::builder(message_store)
        .minimum_level(Level::Debug)
        .build();

    let session_store_clone = session_store.clone();
    actix_web::rt::task::spawn_blocking(move || async move {
        let mut interval = time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            let result = session_store_clone.cleanup().await;
            println!("300 sec {:?}", result);
        }
    });

    let server = HttpServer::new(move || {
        App::new()
            .wrap(message_framework.clone())
            .wrap(TracingLogger::default())
            .wrap(SessionMiddleware::new(
                session_store.clone(),
                secret_key.clone(),
            ))
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
            .route("/", web::get().to(site::home::home))
            .route("/login", web::get().to(site::login::get::login_form))
            .route("/login", web::post().to(site::login::post::post))
            .service(
                web::scope("/admin")
                    .wrap(from_fn(reject_anonymous_users))
                    .route(
                        "/dashboard",
                        web::get().to(site::admin::dashboard::admin_dashboard),
                    )
                    .route(
                        "/password",
                        web::get().to(site::admin::password::get::change_password_form),
                    )
                    .route(
                        "/password",
                        web::post().to(site::admin::password::post::change_password),
                    )
                    .route("/logout", web::post().to(site::admin::logout::log_out))
                    .route(
                        "/newsletters",
                        web::get().to(site::admin::newsletter::get::get),
                    )
                    .route(
                        "/newsletters",
                        web::post().to(site::admin::newsletter::post::publish_newsletter),
                    ),
            )
            .app_data(db_pool_web.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
            .app_data(web::Data::new(HmacSecret(hmac_secret.clone())))
    })
    .listen(listener)?
    .run();

    Ok(server)
}

#[derive(Clone)]
pub struct HmacSecret(pub Secret<String>);

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
