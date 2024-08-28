use std::time::Duration;

use anyhow::Context;
use sqlx::{Sqlite, SqlitePool, Transaction};
use tracing::{field::display, Span};

use crate::{
    configuration::Settings, domain::subscriber_email::SubscriberEmail, email_client::EmailClient,
    utils::get_connection_pool,
};

pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

#[tracing::instrument(
    skip_all,
    fields(
        newsletter_issue_id=tracing::field::Empty,
        subscriber_email=tracing::field::Empty
    ),
    err
)]
pub async fn try_execute_task(
    pool: &SqlitePool,
    email_client: &EmailClient,
) -> Result<ExecutionOutcome, anyhow::Error> {
    let task = dequeue_task(pool).await?;
    if task.is_none() {
        return Ok(ExecutionOutcome::EmptyQueue);
    }

    let (transaction, issue_id, email) = task.unwrap();

    Span::current()
        .record("newsletter_issue_id", display(&issue_id))
        .record("subscriber_email", display(&email));
    // TODO: Send email
    match SubscriberEmail::parse(email.clone()) {
        Ok(email) => {
            let issue = get_issue(pool, &issue_id).await?;
            if let Err(e) = email_client
                .send_email(
                    &email,
                    &issue.title,
                    &issue.html_content,
                    &issue.text_content,
                )
                .await
            {
                // unlock_job(&mut transaction, &issue_id, &email).await?;
                tracing::error!(
                    error.cause_chain = ?e,
                    error.message = %e,
                    "Failed to deliver issue to a confirmed subscriber. Skipping"
                )
            }
        }
        Err(e) => {
            // unlock_job(&mut transaction, &issue_id, &email).await?;
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "Skipping a confirmed subscriber. Their stored contact details are invalid"
            )
        }
    }
    delete_task(transaction, &issue_id, &email).await?;
    Ok(ExecutionOutcome::TaskCompleted)
}

type SqliteTransaction = Transaction<'static, Sqlite>;

#[tracing::instrument(skip_all)]
async fn dequeue_task(
    pool: &SqlitePool,
) -> Result<Option<(SqliteTransaction, String, String)>, anyhow::Error> {
    let mut transaction = pool.begin().await?;
    let get_job = sqlx::query!(r#"SELECT newsletter_issue_id, subscriber_email, locked from issue_delivery_queue WHERE locked = 0 LIMIT 1"#)
    .fetch_optional(&mut *transaction)
    .await
    .context("No jobs available.")?;

    if let Some(job) = get_job {
        lock_job(
            &mut transaction,
            &job.newsletter_issue_id,
            &job.subscriber_email,
        )
        .await?;
        Ok(Some((
            transaction,
            job.newsletter_issue_id,
            job.subscriber_email,
        )))
    } else {
        Ok(None)
    }
}

#[tracing::instrument(skip_all)]
async fn delete_task(
    mut transaction: SqliteTransaction,
    issue_id: &String,
    email: &str,
) -> Result<(), anyhow::Error> {
    let _delete_job = sqlx::query!(
        r#"
        DELETE FROM issue_delivery_queue
        WHERE
            newsletter_issue_id = $1 AND
            subscriber_email = $2
        "#,
        issue_id,
        email
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

async fn lock_job(
    transaction: &mut SqliteTransaction,
    issue_id: &String,
    subscriber_email: &String,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE issue_delivery_queue
        SET locked = 1
        WHERE newsletter_issue_id = $1 AND subscriber_email = $2
        "#,
        issue_id,
        subscriber_email
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

async fn _unlock_job(
    transaction: &mut SqliteTransaction,
    issue_id: &String,
    subscriber_email: &String,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE issue_delivery_queue
        SET locked = 0
        WHERE newsletter_issue_id = $1 AND subscriber_email = $2
        "#,
        issue_id,
        subscriber_email
    )
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

struct NewsletterIssue {
    title: String,
    text_content: String,
    html_content: String,
}

#[tracing::instrument(skip_all)]
async fn get_issue(pool: &SqlitePool, issue_id: &String) -> Result<NewsletterIssue, anyhow::Error> {
    let issue = sqlx::query_as!(
        NewsletterIssue,
        r#"
        SELECT title, text_content, html_content 
        FROM newsletter_issues 
        WHERE
            newsletter_issue_id = $1
        "#,
        issue_id
    )
    .fetch_one(pool)
    .await?;

    Ok(issue)
}

async fn worker_loop(pool: SqlitePool, email_client: EmailClient) -> Result<(), anyhow::Error> {
    loop {
        match try_execute_task(&pool, &email_client).await {
            Ok(ExecutionOutcome::EmptyQueue) => {
                actix_web::rt::time::sleep(Duration::from_secs(10)).await;
            }
            Err(_) => {
                actix_web::rt::time::sleep(Duration::from_secs(1)).await;
            }
            Ok(ExecutionOutcome::TaskCompleted) => {}
        }
    }
}

pub async fn run_worker_until_stopped(configuration: Settings) -> Result<(), anyhow::Error> {
    let connection_pool = get_connection_pool(&configuration.database, None).await;
    let email_client = configuration.email_client.client();
    worker_loop(connection_pool, email_client).await
}
