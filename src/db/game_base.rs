use std::collections::HashSet;

use chrono::{Duration, Utc};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{Executor, Pool, Postgres, QueryBuilder, types::Json};
use tracing::warn;
use uuid::Uuid;

use crate::{
    config::app_config::CONFIG,
    models::{
        error::ServerError,
        game_base::{GameBase, GamePagedRequest, GameType, PagedResponse},
    },
};

pub async fn increment_times_played(
    pool: &Pool<Postgres>,
    game_id: Uuid,
) -> Result<(), ServerError> {
    let row = sqlx::query!(
        r#"
        UPDATE "game_base"
        SET times_played = times_played + 1
        WHERE id = $1
        "#,
        game_id
    )
    .execute(pool)
    .await?;

    if row.rows_affected() == 0 {
        return Err(ServerError::NotFound(format!(
            "Game with id {} does not exist",
            game_id
        )));
    }

    Ok(())
}

pub async fn create_game_base<'e, E>(executor: E, game: &GameBase) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
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
    .execute(executor)
    .await?;

    if row.rows_affected() == 0 {
        warn!("Skipping game base creation: id already exists");
    }

    Ok(())
}

pub async fn delete_stale_games(
    pool: &Pool<Postgres>,
    retention_days: u16,
) -> Result<u64, sqlx::Error> {
    let cutoff = Utc::now() - Duration::days(retention_days as i64);
    let mut tx = pool.begin().await?;
    let result = sqlx::query!(r#"DELETE FROM "game_base" WHERE last_played < $1"#, cutoff)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(result.rows_affected())
}

pub async fn get_game_page(
    pool: &Pool<Postgres>,
    request: &GamePagedRequest,
) -> Result<PagedResponse<GameBase>, sqlx::Error> {
    let page_size = CONFIG.server.page_size;
    let limit = page_size + 1;
    let page_num = request.page_num.unwrap_or(0);
    let offset = page_size * page_num;

    let mut where_clause = Vec::new();

    if let Some(category) = request.category.clone() {
        where_clause.push(format!("category = '{}'", category));
    }

    if let Some(game_type) = request.game_type {
        where_clause.push(format!("game_type = '{}'", game_type.as_str()));
    }

    let mut query = r#"
        SELECT 
            id,
            name,
            game_type,
            category,
            iterations,
            times_played,
            last_played
        FROM "game_base"
        "#
    .to_string();

    if !where_clause.is_empty() {
        query.push_str(&format!("WHERE {}", where_clause.join(" AND ")));
    }

    query.push_str(&format!(
        r#"
        ORDER BY times_played DESC
        LIMIT {}
        OFFSET {}
    "#,
        limit, offset
    ));

    let mut games = sqlx::query_as::<_, GameBase>(&query)
        .fetch_all(pool)
        .await?;

    let has_next = games.len() > page_size as usize;
    if has_next {
        games.pop();
    }
    let page = PagedResponse {
        page_num,
        items: games,
        has_next,
        has_prev: page_num > 0,
    };

    Ok(page)
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
    let page_num = request.page_num.unwrap_or(0);
    let offset = page_num * page_size;

    let game_type = match request.game_type {
        Some(game_type) => format!(" AND base.game_type = '{}'", game_type.as_str()),
        None => String::new(),
    };

    let query = format!(
        r#"
        SELECT
            base.id,
            base.name,
            base.game_type,
            base.category,
            base.iterations,
            base.times_played,
            base.last_played
        FROM "game_base" base
        JOIN "saved_game" saved
        ON base.id = saved.base_id
        WHERE saved.user_id = $1 {}
        LIMIT {} OFFSET {}
        "#,
        game_type, limit, offset
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
        page_num,
        items: games,
        has_next,
        has_prev: page_num > 0,
    };

    Ok(page)
}

pub async fn get_random_rounds<T>(
    pool: &Pool<Postgres>,
    game_type: GameType,
    num_rounds: i64,
) -> Result<Vec<T>, ServerError>
where
    T: DeserializeOwned + Send + Unpin + 'static,
{
    let rows: Vec<Json<T>> = sqlx::query_scalar(
        r#"
        WITH threshold AS (
            SELECT random() AS cutoff
        )
        SELECT round_json
        FROM round_pool, threshold
        WHERE game_type = $1
        ORDER BY (random_key < threshold.cutoff), random_key
        LIMIT $2
        "#,
    )
    .bind(game_type as GameType)
    .bind(num_rounds)
    .fetch_all(pool)
    .await?;
    let rounds: Vec<T> = rows.into_iter().map(|json| json.0).collect();

    if (rounds.len() as i64) < num_rounds {
        let error = format!(
            "Not enough random rounds for {}, got {}/{} rounds",
            game_type.as_str(),
            rounds.len(),
            num_rounds
        );
        warn!(error);
        return Err(ServerError::Internal(error));
    }

    Ok(rounds)
}

pub async fn fill_rounds_pool<T>(
    pool: &Pool<Postgres>,
    game_type: GameType,
    rounds: Vec<T>,
) -> Result<(), sqlx::Error>
where
    T: Serialize,
{
    if rounds.is_empty() {
        return Ok(());
    }

    let mut seen = HashSet::new();
    let unique_rounds: Vec<serde_json::Value> = rounds
        .into_iter()
        .filter_map(|r| {
            let v = serde_json::to_value(r).ok()?;
            if seen.insert(v.to_string()) {
                Some(v)
            } else {
                None
            }
        })
        .collect();

    if unique_rounds.is_empty() {
        return Ok(());
    }

    let mut query_builder =
        QueryBuilder::<Postgres>::new(r#"INSERT INTO "round_pool" (game_type, round_json) "#);

    query_builder.push_values(unique_rounds, |mut row, round| {
        row.push_bind(game_type).push_bind(Json(round));
    });

    query_builder.push(r#" ON CONFLICT (game_type, round_json) DO NOTHING"#);

    query_builder.build().execute(pool).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::env;

    use dotenvy::dotenv;
    use serde::{Deserialize, Serialize};
    use sqlx::{Pool, Postgres, types::Json};

    use crate::models::game_base::GameType;

    use super::{fill_rounds_pool, get_random_rounds};

    async fn setup_pool() -> Pool<Postgres> {
        dotenv().ok();
        let url = env::var("TERO__DATABASE_URL").expect("TERO__DATABASE_URL not set");
        sqlx::postgres::PgPoolOptions::new()
            .connect(&url)
            .await
            .unwrap()
    }

    async fn seed<T: Serialize>(pool: &Pool<Postgres>, game_type: GameType, items: &[T]) {
        for item in items {
            sqlx::query(r#"INSERT INTO "round_pool" (game_type, round_json) VALUES ($1, $2)"#)
                .bind(game_type)
                .bind(Json(item))
                .execute(pool)
                .await
                .unwrap();
        }
    }

    async fn cleanup(pool: &Pool<Postgres>, game_type: GameType) {
        sqlx::query(r#"DELETE FROM "round_pool" WHERE game_type = $1"#)
            .bind(game_type)
            .execute(pool)
            .await
            .unwrap();
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestRound {
        value: i32,
        label: String,
    }

    #[tokio::test]
    async fn random_rounds_string() {
        if env::var("ENVIRONMENT").unwrap_or_default() != "dev" {
            return;
        }
        let pool = setup_pool().await;
        let game_type = GameType::Quiz;
        cleanup(&pool, game_type).await;

        let items: Vec<String> = (0..6).map(|i| format!("round_{i}")).collect();
        seed(&pool, game_type, &items).await;

        let mut seen: std::collections::HashSet<String> = Default::default();
        for _ in 0..10 {
            let rounds: Vec<String> = get_random_rounds(&pool, game_type, 2).await.unwrap();
            assert_eq!(rounds.len(), 2);
            seen.extend(rounds);
        }
        assert!(
            seen.len() > 2,
            "Expected variety across fetches, got: {seen:?}"
        );

        cleanup(&pool, game_type).await;
    }

    #[tokio::test]
    async fn random_rounds_int() {
        if env::var("ENVIRONMENT").unwrap_or_default() != "dev" {
            return;
        }
        let pool = setup_pool().await;
        let game_type = GameType::Duel;
        cleanup(&pool, game_type).await;

        let items: Vec<i32> = (100..106).collect();
        seed(&pool, game_type, &items).await;

        let mut seen: std::collections::HashSet<i32> = Default::default();
        for _ in 0..10 {
            let rounds: Vec<i32> = get_random_rounds(&pool, game_type, 2).await.unwrap();
            assert_eq!(rounds.len(), 2);
            seen.extend(rounds);
        }
        assert!(
            seen.len() > 2,
            "Expected variety across fetches, got: {seen:?}"
        );

        cleanup(&pool, game_type).await;
    }

    #[tokio::test]
    async fn random_rounds_struct() {
        if env::var("ENVIRONMENT").unwrap_or_default() != "dev" {
            return;
        }
        let pool = setup_pool().await;
        let game_type = GameType::Roulette;
        cleanup(&pool, game_type).await;

        let items: Vec<TestRound> = (0..6)
            .map(|i| TestRound {
                value: i,
                label: format!("label_{i}"),
            })
            .collect();
        seed(&pool, game_type, &items).await;

        let mut seen: std::collections::HashSet<i32> = Default::default();
        for _ in 0..10 {
            let rounds: Vec<TestRound> = get_random_rounds(&pool, game_type, 2).await.unwrap();
            assert_eq!(rounds.len(), 2);
            seen.extend(rounds.iter().map(|r| r.value));
        }
        assert!(
            seen.len() > 2,
            "Expected variety across fetches, got: {seen:?}"
        );

        cleanup(&pool, game_type).await;
    }

    #[tokio::test]
    async fn fill_rounds_pool_skips_duplicates() {
        if env::var("ENVIRONMENT").unwrap_or_default() != "dev" {
            return;
        }

        let pool = setup_pool().await;
        let game_type = GameType::Imposter;
        cleanup(&pool, game_type).await;

        let items = vec![
            String::from("round_a"),
            String::from("round_b"),
            String::from("round_a"),
        ];

        fill_rounds_pool(&pool, game_type, items.clone())
            .await
            .unwrap();
        fill_rounds_pool(&pool, game_type, items).await.unwrap();

        let count: i64 =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "round_pool" WHERE game_type = $1"#)
                .bind(game_type)
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(count, 2);

        cleanup(&pool, game_type).await;
    }
}
