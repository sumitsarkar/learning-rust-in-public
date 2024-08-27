use std::time::Duration;

use sqlx::SqlitePool;
use wiremock::{
    matchers::{any, method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::{assert_is_redirect_to, spawn_app, ConfirmationLinks, TestApp};

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
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers(pool: SqlitePool) {
    let app = spawn_app(pool).await;
    create_unconfirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

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
        "text_content": "Newsletter body as plain test",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let response = app.post_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // Act - Part 2 - Follow redirect
    let html_page = app.get_newsletter_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));
}

#[sqlx::test]
async fn newsletters_are_delivered_to_confirmed_subscribers(pool: SqlitePool) {
    let app = spawn_app(pool).await;
    create_confirmed_subscriber(&app).await;

    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "text_content": "Newsletter body as plain test",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let response = app.post_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // Act - Part 2 - Follow redirect
    let html_page = app.get_newsletter_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));
}

#[sqlx::test]
async fn get_newsletter_page(pool: SqlitePool) {
    let app = spawn_app(pool).await;

    // Act - Part 1 - Login
    app.test_user.login(&app).await;

    // Act - Part 2 - Extract Link to Newsletter page
    let html_page = app.get_newsletter_html().await;
    assert!(html_page.contains(r#"<form action="/admin/newsletters" method="post">"#));

    // Act - Part 3 - Submit a new Newsletter
}

#[sqlx::test]
async fn you_must_be_logged_in_to_see_the_newsletter_form(pool: SqlitePool) {
    let app = spawn_app(pool).await;

    let response = app.get_publish_newsletter().await;

    assert_is_redirect_to(&response, "/login");
}

#[sqlx::test]
async fn you_must_be_logged_in_to_publish_a_newsletter(pool: SqlitePool) {
    let app = spawn_app(pool).await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "text_content": "Newsletter body as plain test",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let response = app.post_newsletters(&newsletter_request_body).await;

    assert_is_redirect_to(&response, "/login");
}

#[sqlx::test]
async fn newsletter_creation_is_idempotent(pool: SqlitePool) {
    let app = spawn_app(pool).await;
    create_confirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // Act - Part 1 - Submit newsletter form
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "text_content": "Newsletter body as plain test",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let response = app.post_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // Act - Part 2 - Follow redirect
    let html_page = app.get_newsletter_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));

    // Act - Part 3 - Submit newsletter form **again**
    let response = app.post_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // Act - Part 4 - Follow the redirect
    let html_page = app.get_newsletter_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));
}

#[sqlx::test]
async fn concurrent_form_submission_is_handled_gracefully(pool: SqlitePool) {
    let app = spawn_app(pool).await;
    create_confirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        // Setting a long delay to ensure that the second request arrives before the first one completes
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // Act - Submit two newsletter forms concurrently
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "text_content": "Newsletter body as plain test",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let response1 = app.post_newsletters(&newsletter_request_body);
    let response2 = app.post_newsletters(&newsletter_request_body);
    let (response1, response2) = tokio::join!(response1, response2);

    assert_eq!(response1.status(), response2.status());
    assert_eq!(
        response1.text().await.unwrap(),
        response2.text().await.unwrap()
    );
}
