use once_cell::sync::Lazy;
use sqlx::SqlitePool;
use std::net::TcpListener;
use zero2prod::{
    startup::run,
    telemetry::{get_subscriber, init_subscriber},
};

#[sqlx::test]
async fn health_check_works(pool: SqlitePool) {
    let test_app = spawn_app(pool).await;

    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/health_check", &test_app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length())
}
#[sqlx::test]
async fn subscribe_returns_a_200_for_valid_form_data(pool: SqlitePool) {
    let test_app = spawn_app(pool).await;

    let client = reqwest::Client::new();

    let body = "name=Potato%20Tomato&email=potato%40tomato.com";
    let response = client
        .post(&format!("{}/subscriptions", &test_app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&test_app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, "potato@tomato.com");
    assert_eq!(saved.name, "Potato Tomato");
}

#[sqlx::test]
async fn subscribe_returns_a_400_when_data_is_missing(pool: SqlitePool) {
    let test_app = spawn_app(pool).await;
    let client = reqwest::Client::new();

    let test_cases = vec![
        ("name=potato", "missing the email"),
        ("email=potato%40tomato.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", &test_app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        )
    }
}

pub struct TestApp {
    pub address: String,
    pub db_pool: SqlitePool,
}

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

async fn spawn_app(pool: SqlitePool) -> TestApp {
    Lazy::force(&TRACING);
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let server = run(listener, pool.clone()).expect("Failed to bind address.");
    let _ = tokio::spawn(server);

    TestApp {
        address,
        db_pool: pool,
    }
}
