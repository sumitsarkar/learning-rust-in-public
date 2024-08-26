use actix_web::{http::header::ContentType, web, HttpResponse};
use anyhow::Context;
use sqlx::SqlitePool;

use crate::{authentication::UserId, utils::e500};

pub async fn admin_dashboard(
    pool: web::Data<SqlitePool>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
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
                <input type="submit" value="Logout">
            </form>
        </li>
        <li><a href="/admin/newsletters">Send a Newsletter</a></li>
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
