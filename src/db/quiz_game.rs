use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::models::{error::ServerError, quiz_game::QuizGame};

pub async fn get_quiz_game_by_id(
    pool: &Pool<Postgres>,
    game_id: &Uuid,
) -> Result<QuizGame, ServerError> {
    let game = sqlx::query_as!(
        QuizGame,
        r#"
        SELECT id, rounds 
        FROM "quiz_game" 
        WHERE id = $1
        "#,
        game_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(ServerError::NotFound(format!(
        "Quiz with id {} does not exist",
        game_id
    )))?;

    Ok(game)
}

pub async fn create_quiz_game(pool: &Pool<Postgres>, game: &QuizGame) -> Result<(), ServerError> {
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
    .execute(pool)
    .await?;

    Ok(())
}
