use chrono::Utc;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::{
    config::config::CONFIG,
    models::{
        error::ServerError,
        game_tip::{CreateGameTipRequest, GameTip},
        popup_manager::PagedResponse,
    },
    service::db_query_builder::DBQueryBuilder,
};

pub async fn create_game_tip(
    pool: &Pool<Postgres>,
    request: &CreateGameTipRequest,
) -> Result<Uuid, ServerError> {
    let id = Uuid::new_v4();
    let created_at = Utc::now();

    let row = sqlx::query(
        r#"
        INSERT INTO "game_tip" (id, header, mobile_phone, description, created_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(id)
    .bind(&request.header)
    .bind(&request.mobile_phone)
    .bind(&request.description)
    .bind(created_at)
    .execute(pool)
    .await?;

    if row.rows_affected() == 0 {
        return Err(ServerError::Internal("Failed to create game tip".into()));
    }

    Ok(id)
}

pub async fn get_game_tips_page(
    pool: &Pool<Postgres>,
    page_num: u16,
) -> Result<PagedResponse<GameTip>, sqlx::Error> {
    let page_size = CONFIG.server.page_size as u16;
    
    let tips = DBQueryBuilder::select(
        r#"
            id,
            header,
            mobile_phone,
            description,
            created_at
        "#,
    )
    .from("game_tip")
    .offset(page_size * page_num)
    .limit(page_size + 1)
    .order_desc("created_at")
    .build()
    .build_query_as::<GameTip>()
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
