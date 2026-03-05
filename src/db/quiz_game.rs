use sqlx::{Executor, Pool, Postgres};
use uuid::Uuid;

use crate::models::{error::ServerError, quiz_game::QuizGame};

pub async fn get_quiz_game_by_id(
    pool: &Pool<Postgres>,
    game_id: Uuid,
) -> Result<QuizGame, sqlx::Error> {
    sqlx::query_as!(
        QuizGame,
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

pub async fn create_quiz_game<'e, E>(executor: E, game: &QuizGame) -> Result<(), ServerError>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query!(
        r#"
        INSERT INTO "quiz_game" (id, rounds)
        VALUES ($1, $2)
        ON CONFLICT (id) DO UPDATE SET
            rounds = EXCLUDED.rounds
        "#,
        game.id,
        &game.rounds
    )
    .execute(executor)
    .await?;

    Ok(())
}
