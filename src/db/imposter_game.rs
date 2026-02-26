use sqlx::{Pool, Postgres};
use tracing::warn;
use uuid::Uuid;

use crate::models::imposter_game::ImposterGame;

pub async fn create_imposter_game(
    pool: &Pool<Postgres>,
    game: &ImposterGame,
) -> Result<(), sqlx::Error> {
    let row = sqlx::query!(
        r#"
        INSERT INTO "imposter_game" ("id", "rounds")
        VALUES ($1, $2); 
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

pub async fn get_imposter_game_by_id(
    pool: &Pool<Postgres>,
    game_id: Uuid,
) -> Result<ImposterGame, sqlx::Error> {
    sqlx::query_as!(
        ImposterGame,
        r#"
        SELECT id, rounds 
        FROM "imposter_game" 
        WHERE id = $1
        "#,
        game_id
    )
    .fetch_one(pool)
    .await
}
