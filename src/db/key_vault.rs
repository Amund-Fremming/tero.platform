use sqlx::{Pool, Postgres};

pub async fn get_word_sets(
    pool: &Pool<Postgres>,
) -> Result<(Vec<String>, Vec<String>), sqlx::Error> {
    let prefix_fut = sqlx::query_scalar!("SELECT word FROM prefix_word").fetch_all(pool);

    let suffix_fut = sqlx::query_scalar!("SELECT word FROM suffix_word").fetch_all(pool);

    let (prefix_result, suffix_result): (
        Result<Vec<String>, sqlx::Error>,
        Result<Vec<String>, sqlx::Error>,
    ) = tokio::join!(prefix_fut, suffix_fut);

    Ok((prefix_result?, suffix_result?))
}
