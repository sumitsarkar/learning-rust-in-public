use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::{Sqlite, SqlitePool, Transaction};
use tsid::create_tsid;

use crate::{
    authentication::UserId,
    idempotency::{save_response, try_processing, IdempotencyKey, NextAction},
    utils::{e400, e500, see_other},
};

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
}

#[tracing::instrument {
    name = "Publish a newsletter issue"
    skip(form, pool)
    fields(user_id=%&*user_id)
}]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    pool: web::Data<SqlitePool>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id: UserId = user_id.into_inner();

    let FormData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;

    // Return early if we have a saved response in the database
    let mut transaction = match try_processing(&pool, &idempotency_key, &user_id)
        .await
        .map_err(e500)?
    {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            return Ok(saved_response);
        }
    };

    tracing::Span::current().record("user_id", tracing::field::display(&user_id));

    let issue_id = insert_newsletter_issue(&mut transaction, &title, &text_content, &html_content)
        .await
        .context("Failed to store newsletter issue details")
        .map_err(e500)?;
    enqueue_delivery_tasks(&mut transaction, issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(e500)?;

    let response = see_other("/admin/newsletters");
    let response = save_response(transaction, &idempotency_key, &user_id, response)
        .await
        .map_err(e500)?;
    success_message().send();
    Ok(response)
}

fn success_message() -> FlashMessage {
    FlashMessage::error("The newsletter issue has been accepted - emails will go out shortly.")
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'static, Sqlite>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<String, sqlx::Error> {
    let newsletter_issue_id = create_tsid().to_string();
    let _ = sqlx::query!(
        r#"
        INSERT INTO newsletter_issues (
            newsletter_issue_id,
            title,
            text_content,
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, unixepoch())
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content
    )
    .execute(&mut **transaction)
    .await?;

    Ok(newsletter_issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'static, Sqlite>,
    newsletters_issue_id: String,
) -> Result<(), sqlx::Error> {
    let _ = sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_id,
            subscriber_email
        )
        SELECT $1, email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletters_issue_id
    )
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
