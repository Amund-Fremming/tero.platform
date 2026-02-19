use chrono::{Duration, Utc};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sqlx::{Pool, Postgres};
use tracing::warn;
use uuid::Uuid;

use crate::{
    config::app_config::CONFIG,
    models::{
        error::ServerError,
        game_base::{
            DeleteGameResult, GameBase, GamePagedRequest, GameType, PagedResponse, RandomGame,
        },
    },
};

pub async fn create_game_base(pool: &Pool<Postgres>, game: &GameBase) -> Result<(), sqlx::Error> {
    // newly created games are not played
    let times_played = 0;
    let row = sqlx::query!(
        r#"
        INSERT INTO "game_base" (id, name, game_type, category, iterations, times_played, last_played)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        game.id,
        game.name,
        game.game_type as _,
        game.category as _,
        game.iterations,
        times_played,
        game.last_played
    )
    .execute(pool)
    .await?;

    if row.rows_affected() == 0 {
        warn!("Skipping game base creation: id already exists");
    }

    Ok(())
}

pub async fn delete_non_active_games(pool: &Pool<Postgres>) -> Result<(), sqlx::Error> {
    let timeout = Utc::now() - Duration::days(CONFIG.server.active_game_retention as i64);
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
    request: &GamePagedRequest,
) -> Result<PagedResponse<GameBase>, sqlx::Error> {
    let page_size = CONFIG.server.page_size as u16;
    let limit = page_size + 1;
    let offset = page_size * request.page_num.unwrap_or(0);

    let category = match &request.category {
        Some(category) => format!("AND category = '{}'", category),
        None => "".to_string(),
    };

    let query = format!(
        r#"
        SELECT 
            id,
            name,
            game_type,
            category,
            iterations,
            times_played,
            last_played,
            synced
        FROM "game_base"
        WHERE game_type = '{}' {} AND synced = true
        ORDER BY times_played DESC
        LIMIT {} OFFSET {}
        "#,
        request.game_type.unwrap_or(GameType::Quiz).as_str(),
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
    let page = PagedResponse {
        page_num: request.page_num.unwrap_or(0),
        items: games,
        has_next,
        has_prev: request.page_num.unwrap_or(0) > 0,
    };

    Ok(page)
}

pub async fn sync_and_update_base(
    pool: &Pool<Postgres>,
    game_id: Uuid,
    iterations: Option<usize>,
) -> Result<(), ServerError> {
    let iterations_condition = match iterations {
        Some(iterations) => format!(", iterations = {}", iterations),
        None => String::new(),
    };

    let query = format!(
        r#"
        UPDATE "game_base"
        SET times_played = times_played + 1, last_played = '{}', synced = true{}
        WHERE id = '{}'
        "#,
        Utc::now(),
        iterations_condition,
        game_id
    );

    let row = sqlx::query(&query).execute(pool).await?;

    if row.rows_affected() == 0 {
        return Err(ServerError::NotFound(format!(
            "Game with id {} does not exist",
            game_id
        )));
    }

    Ok(())
}

pub async fn delete_game(pool: &Pool<Postgres>, id: Uuid) -> Result<DeleteGameResult, sqlx::Error> {
    let row = sqlx::query_as::<_, DeleteGameResult>(
        r#"
        DELETE FROM "game_base"
        WHERE id = $1
        RETURNING game_type, category
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(DeleteGameResult {
        game_type: row.game_type,
        category: row.category,
    })
}

pub async fn save_game(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    game_id: Uuid,
) -> Result<(), ServerError> {
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
        warn!("User has already saved this game or game does not exist");
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
        return Err(ServerError::NotFound(format!(
            "Saved game for user {} and game {} not found",
            user_id, game_id
        )));
    }

    Ok(())
}

pub async fn get_saved_games_page(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    request: GamePagedRequest,
) -> Result<PagedResponse<GameBase>, ServerError> {
    let page_size = CONFIG.server.page_size;
    let limit = page_size + 1;
    let offset = request.page_num.unwrap_or(0) * page_size;
    let game_type = request.game_type.unwrap_or(GameType::Quiz);

    let query = format!(
        r#"
        SELECT
            base.id,
            base.name,
            base.game_type,
            base.category,
            base.iterations,
            base.times_played,
            base.last_played,
            base.synced
        FROM "game_base" base
        JOIN "saved_game" saved
        ON base.id = saved.base_id
        WHERE synced = true AND saved.user_id = $1 AND base.game_type = $2
        LIMIT {} OFFSET {}
        "#,
        limit, offset
    );

    let mut games = sqlx::query_as::<_, GameBase>(&query)
        .bind(user_id)
        .bind(game_type)
        .fetch_all(pool)
        .await?;

    let has_next = games.len() > limit as usize;
    if has_next {
        games.pop();
    }
    let page = PagedResponse {
        page_num: request.page_num.unwrap_or(0) as u16,
        items: games,
        has_next,
        has_prev: request.page_num.unwrap_or(0) > 0,
    };

    Ok(page)
}

pub async fn take_random_game(
    pool: &Pool<Postgres>,
    game_type: &GameType,
) -> Result<RandomGame, sqlx::Error> {
    let mut rng = ChaCha8Rng::from_os_rng();
    let random_id = rng.random_range(4..=7);

    // TODO! get biggest id by getting last inserted, then get 5-10 random ids, just so i get one with one db trip, if not found, try again

    sqlx::query_as!(
        RandomGame,
        r#"
        DELETE FROM "random_game"
        WHERE id = $1 AND game_type = $2
        RETURNING id, game_id, rounds, game_type AS "game_type: GameType" 
    "#,
        random_id,
        game_type as _
    )
    .fetch_one(pool)
    .await
}
