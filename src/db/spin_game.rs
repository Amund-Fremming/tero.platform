use chrono::Utc;
use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use crate::models::{
    error::ServerError,
    spin_game::{SpinGame, SpinSession},
};

pub async fn get_spin_session_by_game_id(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    game_id: Uuid,
) -> Result<SpinSession, ServerError> {
    let game = sqlx::query_as!(
        SpinGame,
        r#"
        SELECT
            base.id AS base_id,
            spin.id AS spin_id,
            base.name,
            base.description,
            base.category AS "category: _",
            base.iterations,
            base.times_played,
            base.last_played,
            spin.rounds
        FROM "game_base" base
        JOIN "spin_game" spin
        ON base.id = spin.base_id
        WHERE base.id = $1
        "#,
        game_id
    )
    .fetch_one(pool)
    .await?;

    let session = SpinSession::from_game(user_id, game);
    Ok(session)
}

// TODO - update this
pub async fn tx_persist_spin_session(
    tx: &mut Transaction<'_, Postgres>,
    session: &SpinSession,
) -> Result<(), ServerError> {
    let last_played = Utc::now();
    let game_row = sqlx::query!(
        r#"
        INSERT INTO "game_base" (id, name, description, category, iterations, times_played, last_played)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        session.base_id,
        session.name,
        session.description,
        session.category as _,
        session.iterations,
        1,
        last_played
    )
    .execute(&mut **tx)
    .await?;

    let spin_id = Uuid::new_v4();
    let round_row = sqlx::query!(
        r#"
        INSERT INTO "spin_game" (id, base_id, rounds)
        VALUES ($1, $2, $3)
        "#,
        spin_id,
        session.base_id,
        &session.rounds
    )
    .execute(&mut **tx)
    .await?;

    if game_row.rows_affected() == 0 || round_row.rows_affected() == 0 {
        return Err(ServerError::Internal(
            "Failed to persist spin game session".into(),
        ));
    }

    Ok(())
}
