use actix_web::{body::to_bytes, http::StatusCode, HttpResponse};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, Sqlite, SqlitePool, Transaction};

use crate::utils::e500;

use super::IdempotencyKey;

#[derive(Deserialize, Serialize, Debug)]
struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

#[derive(sqlx::FromRow, Debug)]
struct IdempotencyRecord {
    response_status_code: u16,
    response_headers: sqlx::types::Json<Vec<HeaderPairRecord>>,
    response_body: Vec<u8>,
}

pub async fn get_saved_response(
    pool: &SqlitePool,
    idempotency_key: &IdempotencyKey,
    user_id: &String,
) -> Result<Option<HttpResponse>, anyhow::Error> {
    let idempotency = idempotency_key.as_ref();
    let saved_response = sqlx::query!(
        r#"
        SELECT 
            response_status_code as "response_status_code!",
            response_headers as "response_headers!",
            response_body "response_body!" 
        FROM idempotency
        WHERE
            user_id = $1 AND
            idempotency_key = $2
        "#,
        user_id,
        idempotency
    )
    .fetch_optional(pool)
    .await?;

    if let Some(r) = saved_response {
        let status_code = StatusCode::from_u16(r.response_status_code.try_into()?)?;
        let mut response = HttpResponse::build(status_code);
        let response_headers: Vec<HeaderPairRecord> = serde_json::from_str(&r.response_headers)?;
        for HeaderPairRecord { name, value } in response_headers {
            response.append_header((name, value));
        }
        Ok(Some(response.body(r.response_body)))
    } else {
        Ok(None)
    }
}

pub async fn save_response(
    mut transaction: Transaction<'static, Sqlite>,
    idempotency_key: &IdempotencyKey,
    user_id: &String,
    http_response: HttpResponse,
) -> Result<HttpResponse, anyhow::Error> {
    let (response_head, body) = http_response.into_parts();
    let body = to_bytes(body).await.map_err(|e| anyhow::anyhow!("{}", e))?;

    let status_code = response_head.status().as_u16() as i16;
    let headers = {
        let mut h = Vec::with_capacity(response_head.headers().len());
        for (name, value) in response_head.headers().iter() {
            let name = name.as_str().to_owned();
            let value = value.as_bytes().to_owned();
            h.push(HeaderPairRecord { name, value });
        }
        h
    };

    sqlx::query(
        r#"
        UPDATE idempotency 
        SET
            response_status_code = $3,
            response_headers = $4,
            response_body = $5
        WHERE
            user_id = $1 AND
            idempotency_key = $2
    "#,
    )
    .bind(user_id)
    .bind(idempotency_key.as_ref())
    .bind(status_code)
    .bind(Json(headers))
    .bind(body.as_ref())
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    let http_response = response_head.set_body(body).map_into_boxed_body();
    Ok(http_response)
}

pub enum NextAction {
    StartProcessing(Transaction<'static, Sqlite>),
    ReturnSavedResponse(HttpResponse),
}

pub async fn try_processing(
    pool: &SqlitePool,
    idempotency_key: &IdempotencyKey,
    user_id: &String,
) -> Result<NextAction, anyhow::Error> {
    let mut transaction = pool.begin().await?;
    let n_inserted_rows = sqlx::query(
        r#"
        INSERT INTO idempotency (
            user_id,
            idempotency_key,
            created_at
        )
        VALUES ($1, $2, unixepoch())
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(idempotency_key.as_ref())
    .execute(&mut *transaction)
    .await?
    .rows_affected();

    if n_inserted_rows > 0 {
        Ok(NextAction::StartProcessing(transaction))
    } else {
        let saved_response = get_saved_response(pool, idempotency_key, user_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("We expected a saved response, we didn't find it."))?;
        Ok(NextAction::ReturnSavedResponse(saved_response))
    }
}
