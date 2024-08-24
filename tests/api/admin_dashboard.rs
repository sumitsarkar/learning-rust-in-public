use sqlx::SqlitePool;

use crate::helpers::{assert_is_redirect_to, spawn_app};

#[sqlx::test]
async fn you_must_be_logged_in_to_access_the_admin_dashboard(pool: SqlitePool) {
    let app = spawn_app(pool).await;

    let response = app.get_admin_dashboard().await;
    assert_is_redirect_to(&response, "/login");
}
