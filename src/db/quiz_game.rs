use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::models::{
    error::ServerError,
    quiz_game::{QuizGame, QuizSession},
};

pub async fn get_quiz_game_by_id(
    pool: &Pool<Postgres>,
    game_id: &Uuid,
) -> Result<QuizGame, ServerError> {
    let game = sqlx::query_as!(
        QuizGame,
        r#"
        SELECT id, questions
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

pub async fn create_quiz_game(
    pool: &Pool<Postgres>,
    session: &QuizSession,
) -> Result<(), ServerError> {
    sqlx::query!(
        r#"
        INSERT INTO "quiz_game" (id, questions)
        VALUES ($1, $2)
        ON CONFLICT (id) DO UPDATE SET
            questions = EXCLUDED.questions
        "#,
        session.game_id,
        &session.questions
    )
    .execute(pool)
    .await?;

    Ok(())
}
