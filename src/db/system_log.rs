use chrono::Utc;
use sqlx::{Pool, Postgres};

use crate::{
    config::app_config::CONFIG,
    models::{
        error::ServerError,
        system_log::{
            LogAction, LogCategoryCount, LogCeverity, SubjectType, SyslogPageQuery, SystemLog,
        },
    },
    service::popup_manager::PagedResponse,
};

pub async fn get_system_log_page(
    pool: &Pool<Postgres>,
    request: SyslogPageQuery,
) -> Result<PagedResponse<SystemLog>, sqlx::Error> {
    let page_size = CONFIG.server.page_size as u16;
    let offset = (page_size * request.page_num.unwrap_or(0)) as i64;
    let limit = (page_size + 1) as i64;

    let mut query = r#"
        SELECT 
            id,
            subject_id,
            subject_type,
            action,
            ceverity,
            function,
            description,
            metadata,
            created_at
        FROM system_log 
    "#
    .to_string();

    let mut where_clause = Vec::new();

    if let Some(subject_type) = request.subject_type {
        where_clause.push(format!(" subject_type = '{}'", subject_type));
    }

    if let Some(action) = request.action {
        where_clause.push(format!(" action = '{}'", action));
    }

    if let Some(ceverity) = request.ceverity {
        where_clause.push(format!(" ceverity = '{}'", ceverity));
    }

    if !where_clause.is_empty() {
        let conditions = where_clause.join(" AND ");
        query.push_str(&format!("WHERE {}", conditions));
    }

    query.push_str(&format!(
        r#"
        ORDER BY created_at DESC
        LIMIT {} OFFSET {} 
    "#,
        limit, offset
    ));

    let logs = sqlx::query_as::<_, SystemLog>(&query)
        .fetch_all(pool)
        .await?;

    let has_next = logs.len() >= page_size as usize;
    let mut items = logs;
    if has_next {
        items.pop();
    }

    let page = PagedResponse::new(items, has_next);

    Ok(page)
}

#[allow(clippy::too_many_arguments)] // TODO
pub async fn create_system_log(
    pool: &Pool<Postgres>,
    subject_id: &str,
    subject_type: &SubjectType,
    action: &LogAction,
    ceverity: &LogCeverity,
    function: &str,
    description: &str,
    metadata: &Option<serde_json::Value>,
) -> Result<(), ServerError> {
    let created_at = Utc::now();
    let row = sqlx::query!(
        r#"
        INSERT INTO "system_log" (subject_id, subject_type, action, ceverity, function, description, metadata, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
        subject_id,
        subject_type as _,
        action as _,
        ceverity as _,
        function,
        description,
        metadata as _,
        created_at
    )
    .execute(pool)
    .await?;

    if row.rows_affected() == 0 {
        return Err(ServerError::Internal("Failed to create system log".into()));
    }

    Ok(())
}

pub async fn get_log_category_count(
    pool: &Pool<Postgres>,
) -> Result<LogCategoryCount, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct CountRow {
        info: i64,
        warning: i64,
        critical: i64,
    }

    let result = sqlx::query_as::<_, CountRow>(
        r#"
        SELECT 
            COUNT(*) FILTER (WHERE ceverity = 'info') as info,
            COUNT(*) FILTER (WHERE ceverity = 'warning') as warning,
            COUNT(*) FILTER (WHERE ceverity = 'critical') as critical
        FROM system_log
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(LogCategoryCount {
        info: result.info,
        warning: result.warning,
        critical: result.critical,
    })
}
