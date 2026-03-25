use sqlx::{Executor, Pool, Postgres};
use tracing::warn;
use uuid::Uuid;

use crate::models::guess_game::GuessGame;

pub async fn get_guess_game_by_id(
    pool: &Pool<Postgres>,
    game_id: Uuid,
) -> Result<GuessGame, sqlx::Error> {
    sqlx::query_as!(
        GuessGame,
        r#"
        SELECT id, rounds 
        FROM "quiz_game" 
        WHERE id = $1
        "#,
        game_id
    )
    .fetch_one(pool)
    .await
}

pub async fn create_guess_game<'e, E>(executor: E, game: &GuessGame) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    let row = sqlx::query!(
        r#"
        INSERT INTO "guess_game" (id, rounds)
        VALUES ($1, $2)
        "#,
        game.id,
        &game.rounds
    )
    .execute(executor)
    .await?;

    if row.rows_affected() == 0 {
        warn!("Skipping guess game creation: id already exists");
    }

    Ok(())
}
