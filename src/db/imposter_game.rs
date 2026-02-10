use sqlx::{Pool, Postgres};
use tracing::warn;

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
