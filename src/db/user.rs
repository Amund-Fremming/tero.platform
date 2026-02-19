use chrono::Utc;
use serde_json::json;
use sqlx::{Pool, Postgres, QueryBuilder, Transaction};
use tracing::warn;
use uuid::Uuid;

use crate::{
    config::app_config::CONFIG,
    models::{
        error::ServerError,
        game_base::{Gender, PagedResponse},
        system_log::{LogAction, LogCeverity},
        user::{
            ActivityStats, Auth0User, AverageUserStats, BaseUser, ListUsersQuery, PatchUserRequest,
            RecentUserStats,
        },
    },
    service::system_log_builder::SystemLogBuilder,
};

pub async fn delete_pseudo_user(pool: &Pool<Postgres>, id: Uuid) -> Result<bool, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        DELETE FROM "pseudo_user"
        WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await?;

    Ok(row.rows_affected() == 0)
}

pub async fn create_pseudo_user(pool: &Pool<Postgres>) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    let last_active = Utc::now();
    sqlx::query_scalar!(
        r#"
        INSERT INTO "pseudo_user" (id, last_active)
        VALUES ($1, $2)
        RETURNING id
        "#,
        id,
        last_active
    )
    .fetch_one(pool)
    .await
}

pub async fn tx_create_pseudo_user(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    let last_active = Utc::now();
    sqlx::query_scalar!(
        r#"
        INSERT INTO "pseudo_user" (id, last_active)
        VALUES ($1, $2)
        RETURNING id
        "#,
        id,
        last_active
    )
    .fetch_one(&mut **tx)
    .await
}

/// NOTE: Only db function allowed to write system logs
pub async fn ensure_pseudo_user(pool: &Pool<Postgres>, id: Uuid) {
    let last_active = Utc::now();
    let result = sqlx::query!(
        r#"
        INSERT INTO "pseudo_user" (id, last_active)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING
        "#,
        id,
        last_active
    )
    .execute(pool)
    .await;

    match result {
        Err(e) => {
            _ = SystemLogBuilder::new(pool)
                .action(LogAction::Create)
                .ceverity(LogCeverity::Critical)
                .function("ensure_psuedo_user")
                .description("Failed to do insert on pseudo user. Should not fail")
                .metadata(json!({"error": e.to_string()}))
                .log();
            warn!("Failed to ensure pseudo user exists for id {}: {}", id, e);
        }
        Ok(row) => {
            if row.rows_affected() != 0 {
                _ = SystemLogBuilder::new(pool)
                    .action(LogAction::Create)
                    .ceverity(LogCeverity::Warning)
                    .function("ensure_psuedo_user")
                    .description("User had pseudo user that did not exist, so a new was created. This will cause ghost users")
                    .log();
                warn!(
                    "Pseudo user {} did not exist and was created - potential ghost user",
                    id
                );
            }
        }
    };
}

pub async fn get_base_user_by_auth0_id(
    pool: &Pool<Postgres>,
    auth0_id: &str,
) -> Result<Option<BaseUser>, sqlx::Error> {
    sqlx::query_as!(
        BaseUser,
        r#"
        SELECT id, username, auth0_id, birth_date, gender as "gender: _", email,
            email_verified, family_name, updated_at, given_name, created_at
        FROM "base_user"
        WHERE auth0_id = $1
        "#,
        auth0_id
    )
    .fetch_optional(pool)
    .await
}

pub async fn get_base_user_by_id(
    pool: &Pool<Postgres>,
    user_id: Uuid,
) -> Result<Option<BaseUser>, sqlx::Error> {
    sqlx::query_as!(
        BaseUser,
        r#"
        SELECT id, username, auth0_id, birth_date, gender as "gender: _", email,
            email_verified, family_name, updated_at, given_name, created_at
        FROM "base_user"
        WHERE id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await
}

pub async fn pseudo_user_exists(pool: &Pool<Postgres>, id: Uuid) -> Result<bool, sqlx::Error> {
    let exists = sqlx::query_scalar!("SELECT id FROM pseudo_user WHERE id = $1", id)
        .fetch_optional(pool)
        .await?;

    Ok(exists.is_some())
}

pub async fn create_base_user(
    tx: &mut Transaction<'_, Postgres>,
    auth0_user: &Auth0User,
) -> Result<Uuid, ServerError> {
    let email = auth0_user.email.clone().unwrap_or("Kenneth".to_string());
    let split = email.split('@').next().unwrap_or("Kenneth").to_string();

    let username = match &auth0_user.username {
        Some(username) => username.to_string(),
        None => split,
    };

    // Extract names safely, with fallbacks to username split
    let given_name: &str = auth0_user
        .given_name
        .as_deref()
        .unwrap_or_else(|| username.split('.').next().unwrap_or("John"));

    let family_name: &str = auth0_user
        .family_name
        .as_deref()
        .unwrap_or_else(|| username.split('.').nth(1).unwrap_or("Doe"));

    let id = Uuid::new_v4();
    let gender = Gender::Unknown;
    let email_value = auth0_user
        .email
        .clone()
        .unwrap_or(format!("{}@mail.com", Uuid::new_v4()));

    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO "base_user" (id, username, auth0_id, gender, email, email_verified, updated_at, family_name, given_name, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        "#,
        id,
        username,
        auth0_user.auth0_id,
        gender as _,
        email_value,
        auth0_user.email_verified,
        auth0_user.updated_at,
        family_name,
        given_name,
        auth0_user.created_at
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(id)
}

pub async fn update_pseudo_user_activity(
    pool: &Pool<Postgres>,
    id: Uuid,
) -> Result<(), ServerError> {
    let last_active = Utc::now();
    let row = sqlx::query!(
        r#"
        UPDATE "pseudo_user"
        SET last_active = $1
        WHERE id = $2
        "#,
        last_active,
        id
    )
    .execute(pool)
    .await?;

    if row.rows_affected() == 0 {
        return Err(ServerError::NotFound(format!(
            "User with id {} does not exist",
            id
        )));
    }

    Ok(())
}

pub async fn patch_base_user_by_id(
    pool: &Pool<Postgres>,
    user_id: &Uuid,
    request: PatchUserRequest,
) -> Result<BaseUser, ServerError> {
    let mut builder: QueryBuilder<'_, Postgres> = sqlx::QueryBuilder::new("UPDATE base_user SET ");
    let mut separator = builder.separated(", ");

    if let Some(username) = request.username {
        separator
            .push("username = ")
            .push_bind_unseparated(username);
    }

    if let Some(gname) = request.given_name {
        separator.push("given_name = ").push_bind_unseparated(gname);
    }

    if let Some(fname) = request.family_name {
        separator
            .push("family_name = ")
            .push_bind_unseparated(fname);
    }

    if let Some(gender) = request.gender {
        separator.push("gender = ").push_bind_unseparated(gender);
    }

    if let Some(birth_date) = request.birth_date {
        separator
            .push("birth_date = ")
            .push_bind_unseparated(birth_date);
    }

    builder.push(" WHERE id = ").push_bind(user_id); // Also fixed: use 'id', not 'user_id'
    builder.push(" RETURNING id, username, auth0_id, birth_date, gender, email, email_verified, family_name, updated_at, given_name, created_at");
    let result: BaseUser = builder.build_query_as().fetch_one(pool).await?;

    Ok(result)
}

pub async fn list_base_users(
    pool: &Pool<Postgres>,
    request: ListUsersQuery,
) -> Result<PagedResponse<BaseUser>, sqlx::Error> {
    let offset = CONFIG.server.page_size * request.page_num;
    let limit = CONFIG.server.page_size + 1;

    let mut users = sqlx::query_as!(
        BaseUser,
        r#"
        SELECT id, username, auth0_id, birth_date, gender as "gender: _", email, email_verified, updated_at, family_name, given_name, created_at
        FROM "base_user"
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit as i64,
        offset as i64
    )
    .fetch_all(pool)
    .await?;

    let has_next = users.len() > CONFIG.server.page_size as usize;
    if has_next {
        users.pop();
    }

    let response = PagedResponse {
        page_num: request.page_num,
        items: users,
        has_prev: request.page_num > 0,
        has_next,
    };

    Ok(response)
}

pub async fn get_user_activity_stats(pool: &Pool<Postgres>) -> Result<ActivityStats, sqlx::Error> {
    let recent_fut = sqlx::query_as!(
        RecentUserStats,
        r#"
        SELECT
            COUNT(*) FILTER (WHERE last_active >= date_trunc('month', CURRENT_DATE)) AS "this_month_users!",
            COUNT(*) FILTER (WHERE last_active >= date_trunc('week', CURRENT_DATE)) AS "this_week_users!",
            COUNT(*) FILTER (WHERE last_active >= CURRENT_DATE) AS "todays_users!"
        FROM pseudo_user
        "#
    )
    .fetch_one(pool);

    let average_fut = sqlx::query_as!(
        AverageUserStats,
        r#"
        SELECT
            COALESCE((
                SELECT AVG(cnt)::float8 
                FROM (
                    SELECT COUNT(*) AS cnt 
                    FROM pseudo_user 
                    WHERE last_active >= CURRENT_DATE - INTERVAL '6 months'
                    GROUP BY date_trunc('month', last_active)
                ) t
            ), 0) AS "avg_month_users!",
            COALESCE((
                SELECT AVG(cnt)::float8 
                FROM (
                    SELECT COUNT(*) AS cnt 
                    FROM pseudo_user 
                    WHERE last_active >= CURRENT_DATE - INTERVAL '8 weeks'
                    GROUP BY date_trunc('week', last_active)
                ) t
            ), 0) AS "avg_week_users!",
            COALESCE((
                SELECT AVG(cnt)::float8 
                FROM (
                    SELECT COUNT(*) AS cnt 
                    FROM pseudo_user 
                    WHERE last_active >= CURRENT_DATE - INTERVAL '30 days'
                    GROUP BY last_active::date
                ) t
            ), 0) AS "avg_daily_users!"
        "#,
    )
    .fetch_one(pool);

    let total_game_count_fut =
        sqlx::query_scalar!("SELECT COUNT(*)::bigint as count FROM game_base").fetch_one(pool);

    let total_user_count_fut =
        sqlx::query_scalar!("SELECT COUNT(*)::bigint as count FROM pseudo_user").fetch_one(pool);

    type RecentStatsResult = Result<RecentUserStats, sqlx::Error>;
    type AverageStatsResult = Result<AverageUserStats, sqlx::Error>;
    type StatsResult = Result<Option<i64>, sqlx::Error>;

    let (recent, average, total_game_count, total_user_count): (
        RecentStatsResult,
        AverageStatsResult,
        StatsResult,
        StatsResult,
    ) = tokio::join!(
        recent_fut,
        average_fut,
        total_game_count_fut,
        total_user_count_fut
    );

    Ok(ActivityStats {
        total_game_count: total_game_count?.unwrap_or(0),
        total_user_count: total_user_count?.unwrap_or(0),
        recent: recent?,
        average: average?,
    })
}
