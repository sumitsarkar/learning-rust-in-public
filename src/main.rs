use secrecy::ExposeSecret;
use sqlx::SqlitePool;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::email_client::EmailClient;
use zero2prod::startup::{run, run_migration};
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    let configuration = get_configuration().expect("Failed to read configuration.");

    configuration.database.create_database_if_missing().await;
    let connection_pool =
        SqlitePool::connect_lazy(configuration.database.connection_string().expose_secret())
            .expect("Failed to connect to Sqlite.");

    // Run Migrations
    run_migration(&connection_pool).await;

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
    let listener = TcpListener::bind(address).expect("Failed to bind random port");
    run(listener, connection_pool, email_client)?.await
}
