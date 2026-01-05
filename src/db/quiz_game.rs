use chrono::Utc;
use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use crate::models::{error::ServerError, game_base::GameTable, quiz_game::QuizSession};

pub async fn get_quiz_session_by_id(
    pool: &Pool<Postgres>,
    base_id: &Uuid,
) -> Result<QuizSession, ServerError> {
    let session = sqlx::query_as!(
        QuizSession,
        r#"
        SELECT 
            base.id AS base_id,
            quiz.id AS quiz_id,
            base.name,
            base.description,
            base.category as "category: _",
            base.iterations,
            base.times_played as "times_played!",
            0 AS "current_iteration!",
            quiz.questions
        FROM "game_base" base
        JOIN "quiz_game" quiz
        ON base.id = quiz.base_id
        WHERE base.id = $1
        "#,
        base_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(ServerError::NotFound(format!(
        "Quiz with id {} does not exist",
        base_id
    )))?;

    Ok(session)
}

pub async fn tx_persist_quiz_session(
    tx: &mut Transaction<'_, Postgres>,
    session: &QuizSession,
) -> Result<(), ServerError> {
    let times_played = 1;
    let last_played = Utc::now();
    let game_type = GameTable::Quiz;

    sqlx::query!(
        r#"
        INSERT INTO "game_base" (id, name, game_type, category, iterations, times_played, last_played)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (id) DO UPDATE SET
            times_played = "game_base".times_played + 1,
            last_played = EXCLUDED.last_played
        "#,
        session.base_id,
        session.name,
        &game_type as _,
        session.category as _,
        session.iterations,
        times_played,
        last_played
    )
    .execute(&mut **tx)
    .await?;

    sqlx::query!(
        r#"
        INSERT INTO "quiz_game" (id, base_id, questions)
        VALUES ($1, $2, $3)
        ON CONFLICT (base_id) DO UPDATE SET
            questions = EXCLUDED.questions
        "#,
        session.quiz_id,
        session.base_id,
        &session.questions
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}
