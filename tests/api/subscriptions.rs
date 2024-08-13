use sqlx::SqlitePool;
use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::spawn_app;

#[sqlx::test]
async fn subscribe_returns_a_200_for_valid_form_data(pool: SqlitePool) {
    let app = spawn_app(pool).await;

    let body = "name=Potato%20Tomato&email=potato%40tomato.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let response: reqwest::Response = app.post_subscriptions(body.into()).await;

    assert_eq!(200, response.status().as_u16());
}

#[sqlx::test]
async fn subscribe_persists_the_new_subscriber(pool: SqlitePool) {
    let app = spawn_app(pool).await;
    let body = "name=Potato%20Tomato&email=potato%40tomato.com";
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");
    assert_eq!(saved.email, "potato@tomato.com");
    assert_eq!(saved.name, "Potato Tomato");
    assert_eq!(saved.status, "pending_confirmation");
}

#[sqlx::test]
async fn subscribe_returns_a_400_when_data_is_missing(pool: SqlitePool) {
    let app = spawn_app(pool).await;

    let test_cases = vec![
        ("name=potato", "missing the email"),
        ("email=potato%40tomato.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = app.post_subscriptions(invalid_body.into()).await;

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

    let test_cases = vec![
        ("name=&email=potato%40gmail.com", "empty name"),
        ("name=Potato&email=", "empty email"),
        ("name=Potato&email=definitely-not-an-email", "invalid email"),
    ];

    for (body, description) in test_cases {
        let response = app.post_subscriptions(body.into()).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return a 200 OK when the payload was {}",
            description
        )
    }
}

#[sqlx::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data(pool: SqlitePool) {
    let app = spawn_app(pool).await;
    let body = "name=falana%20dekana&email=falana%40dekana.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(&email_request);

    assert_eq!(confirmation_links.html, confirmation_links.plain_text);
}

#[sqlx::test]
async fn confirmations_without_token_are_rejected_with_a_400(pool: SqlitePool) {
    let app = spawn_app(pool).await;

    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 400);
}

#[sqlx::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called(pool: SqlitePool) {
    let app = spawn_app(pool).await;
    let body = "name=falana%20dekana&email=falana%40dekana.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(&email_request);

    let response = reqwest::get(confirmation_links.html).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
}

#[sqlx::test]
async fn clicking_on_the_confirmation_link_confirms_a_subscriber(pool: SqlitePool) {
    let app = spawn_app(pool).await;
    let body = "name=falana%20dekana&email=falana%40dekana.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(&email_request);

    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let saved = sqlx::query!("SELECT email, name, status from subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to getch saved subscription.");

    assert_eq!(saved.email, "falana@dekana.com");
    assert_eq!(saved.name, "falana dekana");
    assert_eq!(saved.status, "confirmed");
}
