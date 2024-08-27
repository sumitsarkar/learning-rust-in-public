use actix_web::{body::to_bytes, http::StatusCode, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, SqlitePool};

use super::IdempotencyKey;

#[derive(Deserialize, Serialize)]
struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

#[derive(sqlx::FromRow)]
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
    let saved_response = sqlx::query_as::<_, IdempotencyRecord>(
        r#"
        SELECT 
            response_status_code,
            response_headers,
            response_body
        FROM idempotency
        WHERE
            user_id = $1 AND
            idempotency_key = $2
        "#,
    )
    .bind(user_id)
    .bind(idempotency_key.as_ref())
    .fetch_optional(pool)
    .await?;

    if let Some(r) = saved_response {
        let status_code = StatusCode::from_u16(r.response_status_code.try_into()?)?;
        let mut response = HttpResponse::build(status_code);
        for HeaderPairRecord { name, value } in r.response_headers.0 {
            response.append_header((name, value));
        }
        Ok(Some(response.body(r.response_body)))
    } else {
        Ok(None)
    }
}

pub async fn save_response(
    pool: &SqlitePool,
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
        INSERT INTO idempotency (
            user_id,
            idempotency_key,
            response_status_code,
            response_headers,
            response_body,
            created_at
        )
        VALUES ($1, $2, $3, $4, $5, unixepoch())
    "#,
    )
    .bind(user_id)
    .bind(idempotency_key.as_ref())
    .bind(status_code)
    .bind(Json(headers))
    .bind(body.as_ref())
    .execute(pool)
    .await?;

    let http_response = response_head.set_body(body).map_into_boxed_body();
    Ok(http_response)
}
