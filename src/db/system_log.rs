use chrono::Utc;
use sqlx::{Pool, Postgres};

use crate::{
    config::config::CONFIG,
    models::{
        error::ServerError,
        popup_manager::PagedResponse,
        system_log::{LogAction, LogCategoryCount, LogCeverity, SubjectType, SyslogPageQuery, SystemLog},
    },
    service::db_query_builder::DBQueryBuilder,
};

pub async fn get_system_log_page(
    pool: &Pool<Postgres>,
    request: SyslogPageQuery,
) -> Result<PagedResponse<SystemLog>, sqlx::Error> {
    let page_size = CONFIG.server.page_size as u16;
    let offset = (page_size * request.page_num) as i64;
    let limit = (page_size + 1) as i64;
    
    let logs = sqlx::query_as!(
        SystemLog,
        r#"
        SELECT 
            id,
            subject_id,
            subject_type as "subject_type: SubjectType",
            action as "action: LogAction",
            ceverity as "ceverity: LogCeverity",
            function,
            description,
            metadata,
            created_at
        FROM system_log
        WHERE ($1::text IS NULL OR subject_type = $1)
          AND ($2::text IS NULL OR action = $2)
          AND ($3::text IS NULL OR ceverity = $3)
        ORDER BY created_at DESC
        LIMIT $4 OFFSET $5
        "#,
        request.subject_type.as_ref().map(|s| s.to_string()),
        request.action.as_ref().map(|a| a.to_string()),
        request.ceverity.as_ref().map(|c| c.to_string()),
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;

    let has_next = logs.len() >= page_size as usize;
    let mut items = logs;
    if has_next {
        items.truncate(page_size as usize);
    }
    
    let page = PagedResponse::new(items, has_next);

    Ok(page)
}

pub async fn create_system_log(
    pool: &Pool<Postgres>,
    subject_id: &str,
    subject_type: &SubjectType,
    action: &LogAction,
    ceverity: &LogCeverity,
    file_name: &str,
    description: &str,
    metadata: &Option<serde_json::Value>,
) -> Result<(), ServerError> {
    let created_at = Utc::now();
    let row = sqlx::query!(
        r#"
        INSERT INTO "system_log" (subject_id, subject_type, action, ceverity, file_name, description, metadata, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
        subject_id,
        subject_type as _,
        action as _,
        ceverity as _,
        file_name,
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
        "#
    )
    .fetch_one(pool)
    .await?;

    Ok(LogCategoryCount {
        info: result.info,
        warning: result.warning,
        critical: result.critical,
    })
}
