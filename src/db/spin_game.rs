use chrono::Utc;
use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use crate::models::{
    error::ServerError,
    game_base::GameType,
    spin_game::{SpinGame, SpinSession},
};

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

// TODO - update this
pub async fn tx_persist_spin_session(
    tx: &mut Transaction<'_, Postgres>,
    session: &SpinSession,
    game_type: &GameType,
) -> Result<(), ServerError> {
    let last_played = Utc::now();
    let game_row = sqlx::query!(
        r#"
        INSERT INTO "game_base" (id, name, game_type, category, iterations, times_played, last_played)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        session.base_id,
        session.name,
        &game_type as _,
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
        INSERT INTO "spin_game" (id, rounds)
        VALUES ($1, $2)
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
