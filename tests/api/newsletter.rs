use sqlx::SqlitePool;
use wiremock::{
    matchers::{any, method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::{spawn_app, ConfirmationLinks, TestApp};

/// Use the public API of the application under test to create an unconfirmed subscriber.
async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=falana%20dekana&email=falana%40dekana.com";
    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();

    // We now inspect the request received by the mock Postmakr server to retrieve the confirmation link and return it
    let email_req = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();
    app.get_confirmation_links(&email_req)
}

#[sqlx::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers(pool: SqlitePool) {
    let app = spawn_app(pool).await;
    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        // Assert that no requests were fired to Postmark!
        .expect(0)
        .mount(&app.email_server)
        .await;

    // A sketch of the newsletter payload structure.
    // We might change it later on.
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "content": {
            "text": "Newsletter body as plain test",
            "html": "<p>Newsletter body as HTML</p>"
        }
    });

    let response = TestApp::post_newsletters(&app, newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 200);
}

#[sqlx::test]
async fn newsletters_are_delivered_to_confirmed_subscribers(pool: SqlitePool) {
    let app = spawn_app(pool).await;
    create_confirmed_subscriber(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>"
        }
    });

    let response = TestApp::post_newsletters(&app, newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 200);
}

#[sqlx::test]
async fn newsletters_returns_400_for_invalid_data(pool: SqlitePool) {
    let app = spawn_app(pool).await;
    let test_cases = vec![
        (
            serde_json::json!({
                "content": {
                    "text": "Newsletter body as plain text",
                    "html": "<p>Newsletter body as HTML</p>"
                }
            }),
            "missing title",
        ),
        (
            serde_json::json!({"title": "Newsletter"}),
            "missing content",
        ),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = TestApp::post_newsletters(&app, invalid_body).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        )
    }
}

async fn create_confirmed_subscriber(app: &TestApp) {
    // We can then reuse the same helper and add an extra step to actually call the confirmation link!
    let confirmation_link = create_unconfirmed_subscriber(app).await;
    reqwest::get(confirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

#[sqlx::test]
async fn requests_missing_authorization_are_rejected(pool: SqlitePool) {
    let app = spawn_app(pool).await;

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", &app.address))
        .json(&serde_json::json!({
            "title": "Newsletter Title",
            "content": {
                "text": "Newsletter body as plain text",
                "html": "<p>Newsletter body as HTML</p>"
            }
        }))
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(401, response.status().as_u16());
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-Authenticate"]
    );
}
