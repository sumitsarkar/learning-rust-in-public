use sqlx::SqlitePool;

use crate::helpers::spawn_app;

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

#[sqlx::test]
async fn subscribe_returns_a_400_when_fields_are_present_but_empty(pool: SqlitePool) {
    let app = spawn_app(pool).await;
    let client = reqwest::Client::new();

    let test_cases = vec![
        ("name=&email=potato%40gmail.com", "empty name"),
        ("name=Potato&email=", "empty email"),
        ("name=Potato&email=definitely-not-an-email", "invalid email"),
    ];

    for (body, description) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", &app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return a 200 OK when the payload was {}",
            description
        )
    }
}
