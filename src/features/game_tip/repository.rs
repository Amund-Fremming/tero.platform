use chrono::Utc;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::{
    common::{error::ServerError, services::popup_manager::PagedResponse},
    config::config::CONFIG,
    features::game_tip::models::{CreateGameTipRequest, GameTip},
};

pub async fn create_game_tip(
    pool: &Pool<Postgres>,
    request: &CreateGameTipRequest,
) -> Result<Uuid, ServerError> {
    let id = Uuid::new_v4();
    let created_at = Utc::now();

    sqlx::query!(
        r#"
        INSERT INTO "game_tip" (id, header, mobile_phone, description, created_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        id,
        &request.header,
        &request.mobile_phone,
        &request.description,
        created_at
    )
    .execute(pool)
    .await?;

    Ok(id)
}

pub async fn get_game_tips_page(
    pool: &Pool<Postgres>,
    page_num: u16,
) -> Result<PagedResponse<GameTip>, sqlx::Error> {
    let page_size = CONFIG.server.page_size as u16;
    let offset = (page_size * page_num) as i64;
    let limit = (page_size + 1) as i64;

    let tips = sqlx::query_as!(
        GameTip,
        r#"
        SELECT id, header, mobile_phone, description, created_at
        FROM game_tip
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;

    let has_next = tips.len() >= page_size as usize;
    let mut items = tips;
    if has_next {
        items.truncate(page_size as usize);
    }

    let page = PagedResponse::new(items, has_next);

    Ok(page)
}
