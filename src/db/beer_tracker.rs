use sqlx::{Pool, Postgres};

use crate::models::{
    beer_tracker::{BeerTrackerGameRow, BeerTrackerMemberRow},
    error::ServerError,
};

pub async fn create_game(
    pool: &Pool<Postgres>,
    id: &str,
    can_size: f64,
    goal: Option<i32>,
    creator_name: &str,
) -> Result<(), ServerError> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        r#"INSERT INTO beer_tracker_games (id, can_size, goal) VALUES ($1, $2, $3)"#,
        id,
        can_size,
        goal,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"INSERT INTO beer_tracker_members (game_id, name) VALUES ($1, $2)"#,
        id,
        creator_name,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn game_exists(pool: &Pool<Postgres>, id: &str) -> Result<bool, ServerError> {
    let row = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM beer_tracker_games WHERE id = $1) as "exists!""#,
        id
    )
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn get_game(
    pool: &Pool<Postgres>,
    id: &str,
) -> Result<Option<BeerTrackerGameRow>, ServerError> {
    let row = sqlx::query_as!(
        BeerTrackerGameRow,
        r#"SELECT id, can_size, goal FROM beer_tracker_games WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn get_members(
    pool: &Pool<Postgres>,
    game_id: &str,
) -> Result<Vec<BeerTrackerMemberRow>, ServerError> {
    let rows = sqlx::query_as!(
        BeerTrackerMemberRow,
        r#"SELECT game_id, name, count FROM beer_tracker_members WHERE game_id = $1 ORDER BY count DESC"#,
        game_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn member_exists(
    pool: &Pool<Postgres>,
    game_id: &str,
    name: &str,
) -> Result<bool, ServerError> {
    let row = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM beer_tracker_members WHERE game_id = $1 AND name = $2) as "exists!""#,
        game_id,
        name
    )
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn add_member(
    pool: &Pool<Postgres>,
    game_id: &str,
    name: &str,
) -> Result<(), ServerError> {
    sqlx::query!(
        r#"INSERT INTO beer_tracker_members (game_id, name) VALUES ($1, $2)"#,
        game_id,
        name,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn increment_count(
    pool: &Pool<Postgres>,
    game_id: &str,
    name: &str,
    amount: i32,
) -> Result<(), ServerError> {
    sqlx::query!(
        r#"UPDATE beer_tracker_members SET count = count + $3 WHERE game_id = $1 AND name = $2"#,
        game_id,
        name,
        amount,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn remove_member(
    pool: &Pool<Postgres>,
    game_id: &str,
    name: &str,
) -> Result<(), ServerError> {
    sqlx::query!(
        r#"DELETE FROM beer_tracker_members WHERE game_id = $1 AND name = $2"#,
        game_id,
        name,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn member_count(pool: &Pool<Postgres>, game_id: &str) -> Result<i64, ServerError> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM beer_tracker_members WHERE game_id = $1"#,
        game_id
    )
    .fetch_one(pool)
    .await?;

    Ok(count)
}

pub async fn delete_game(pool: &Pool<Postgres>, id: &str) -> Result<(), ServerError> {
    sqlx::query!(r#"DELETE FROM beer_tracker_games WHERE id = $1"#, id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn delete_stale_games(pool: &Pool<Postgres>) -> Result<u64, ServerError> {
    let result: sqlx::postgres::PgQueryResult = sqlx::query!(
        r#"DELETE FROM beer_tracker_games WHERE created_at < NOW() - INTERVAL '24 hours'"#
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
