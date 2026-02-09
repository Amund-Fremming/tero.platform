use sqlx::{Pool, Postgres};

use crate::models::game::GameType;

pub async fn get_random_game(
    pool: &Pool<Postgres>,
    game_type: &GameType,
) -> Result<RandomGame, sqlx::Error> {
    todo!();
}
