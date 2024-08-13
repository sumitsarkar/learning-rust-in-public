use sqlx::SqlitePool;

use crate::helpers::spawn_app;

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
