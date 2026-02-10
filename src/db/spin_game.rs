use sqlx::{Pool, Postgres};
use tracing::warn;
use uuid::Uuid;

use crate::models::spin_game::SpinGame;

pub async fn get_spin_game_by_id(pool: &Pool<Postgres>, id: Uuid) -> Result<SpinGame, sqlx::Error> {
    sqlx::query_as!(
        SpinGame,
        r#"
        SELECT id, rounds
        FROM "spin_game"
        WHERE id = $1
        "#,
        id
    )
    .fetch_one(pool)
    .await
}

pub async fn create_spin_game(pool: &Pool<Postgres>, game: &SpinGame) -> Result<(), sqlx::Error> {
    let row = sqlx::query!(
        r#"
        INSERT INTO "spin_game" (id, rounds)
        VALUES ($1, $2)
        "#,
        game.id,
        &game.rounds
    )
    .execute(pool)
    .await?;

    if row.rows_affected() == 0 {
        warn!("Skipping spin game creation: id already exists");
    }

    Ok(())
}
