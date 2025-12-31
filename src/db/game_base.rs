use chrono::{Duration, Utc};
use sqlx::{Pool, Postgres};
use tracing::warn;
use uuid::Uuid;

use crate::{
    config::config::CONFIG,
    models::{
        error::ServerError,
        game_base::{GameBase, GamePageQuery, GameType, SavedGamesPageQuery},
    },
    service::popup_manager::PagedResponse,
};

pub async fn delete_non_active_games(pool: &Pool<Postgres>) -> Result<(), sqlx::Error> {
    let timeout = Utc::now() - Duration::days(24);
    sqlx::query!(
        r#"
        DELETE FROM "game_base"
        WHERE last_played < $1
        "#,
        timeout
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_game_page(
    pool: &Pool<Postgres>,
    request: &GamePageQuery,
) -> Result<PagedResponse<GameBase>, sqlx::Error> {
    let page_size = CONFIG.server.page_size as u16;
    let limit = page_size + 1;
    let offset = page_size * request.page_num;

    let category = match &request.category {
        Some(category) => format!("AND category = '{}'", category),
        None => "".to_string(),
    };

    let query = format!(
        r#"
        SELECT 
            id,
            name,
            description,
            game_type,
            category,
            iterations,
            times_played,
            last_played
        FROM "game_base"
        WHERE game_type = '{}' {}
        ORDER BY times_played DESC
        LIMIT {} OFFSET {}
        "#,
        request.game_type.as_str(),
        category,
        limit,
        offset
    );

    let mut games = sqlx::query_as::<_, GameBase>(&query)
        .fetch_all(pool)
        .await?;

    let has_next = games.len() > page_size as usize;
    if has_next {
        games.pop();
    }
    let page = PagedResponse::new(games, has_next);

    Ok(page)
}

pub async fn increment_times_played(
    pool: &Pool<Postgres>,
    game_id: Uuid,
) -> Result<(), ServerError> {
    let row = sqlx::query!(
        r#"
        UPDATE "game_base"
        SET times_played = times_played + 1, last_played = $1
        WHERE id = $2
        "#,
        Utc::now(),
        game_id
    )
    .execute(pool)
    .await?;

    if row.rows_affected() == 0 {
        warn!("Query failed, no game with id: {}", game_id);
        return Err(ServerError::NotFound("Game does not exist".into()));
    }

    Ok(())
}

pub async fn delete_game(
    pool: &Pool<Postgres>,
    game_type: &GameType,
    id: Uuid,
) -> Result<(), ServerError> {
    let query = format!(
        r#"
        DELETE FROM {}
        WHERE id = $1
        "#,
        game_type.table_name()
    );

    let row = sqlx::query(&query).bind(id).execute(pool).await?;
    if row.rows_affected() == 0 {
        warn!("Query failed, no game with id: {}", id);
        return Err(ServerError::Internal("Failed to delete game".into()));
    }

    Ok(())
}

pub async fn save_game(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    game_id: Uuid,
) -> Result<(), ServerError> {
    use tracing::warn;
    let id = Uuid::new_v4();
    let row = sqlx::query!(
        r#"
        INSERT INTO "saved_game" (id, user_id, base_id)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, base_id) DO NOTHING
        "#,
        id,
        user_id,
        game_id
    )
    .execute(pool)
    .await?;

    if row.rows_affected() == 0 {
        warn!("User has already saved this game");
    }

    Ok(())
}

pub async fn delete_saved_game(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    game_id: Uuid,
) -> Result<(), ServerError> {
    let row = sqlx::query!(
        r#"
        DELETE FROM "saved_game"
        WHERE user_id = $1 AND base_id = $2
        "#,
        user_id,
        game_id
    )
    .execute(pool)
    .await?;

    if row.rows_affected() == 0 {
        return Err(ServerError::Internal(
            "Failed to delete from table `saved_game`".into(),
        ));
    }

    Ok(())
}

pub async fn get_saved_games_page(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    query: SavedGamesPageQuery,
) -> Result<PagedResponse<GameBase>, ServerError> {
    let page_size = CONFIG.server.page_size;
    let limit = page_size + 1;
    let offset = query.page_num * page_size;

    let query = format!(
        r#"
        SELECT
            base.id,
            base.name,
            base.description,
            base.game_type,
            base.category,
            base.iterations,
            base.times_played,
            base.last_played
        FROM "game_base" base
        JOIN "saved_game" saved
        ON base.id = saved.base_id
        WHERE saved.user_id = $1
        LIMIT {} OFFSET {}
        "#,
        limit, offset
    );

    let mut games = sqlx::query_as::<_, GameBase>(&query)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

    let has_next = games.len() > limit as usize;
    if has_next {
        games.pop();
    }
    let page = PagedResponse::new(games, has_next);

    Ok(page)
}
