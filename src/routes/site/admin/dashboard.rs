use actix_web::{
    http::header::{ContentType, LOCATION},
    web, HttpResponse,
};
use anyhow::Context;
use sqlx::SqlitePool;

use crate::{session_state::TypedSession, utils::e500};

use super::password::post::reject_anonymous_users;

pub async fn admin_dashboard(
    session: TypedSession,
    pool: web::Data<SqlitePool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = reject_anonymous_users(session).await?;
    let username = get_username(&user_id, &pool).await.map_err(e500)?;

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Admin dashboard</title>
</head>
<body>
    <p>Welcome {username}!</p>
    <p>Available actions:</p>
    <ol>
        <li><a href="/admin/password">Change password</a></li>
        <li>
            <form name="logoutForm" action="/admin/logout" method="post">
                <input type="submit" value="logout">
            </form>
        </li>
    </ol>
</body>
</html>"#
        )))
}

#[tracing::instrument {
    name = "Get Username",
    skip(pool)
}]
pub async fn get_username(user_id: &String, pool: &SqlitePool) -> Result<String, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT username
        FROM users
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_one(pool)
    .await
    .context("Failed to perform a query to retrieve a username.")?;
    Ok(row.username)
}
