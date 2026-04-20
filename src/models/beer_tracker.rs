use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BeerTrackerGameRow {
    pub id: String,
    pub can_size: f64,
    pub goal: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BeerTrackerMemberRow {
    pub game_id: String,
    pub name: String,
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeerTrackerGame {
    pub id: String,
    pub can_size: f64,
    pub goal: Option<i32>,
    pub members: Vec<UserScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserScore {
    pub name: String,
    pub count: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateBeerTrackerRequest {
    #[validate(length(min = 2, max = 20, message = "Game ID must be 2-20 characters"))]
    pub game_id: String,
    #[validate(length(min = 1, max = 20, message = "Name must be 1-20 characters"))]
    pub name: String,
    pub can_size: f64,
    pub goal: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct JoinBeerTrackerRequest {
    #[validate(length(min = 1, max = 20, message = "Name must be 1-20 characters"))]
    pub name: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct IncrementBeerRequest {
    #[validate(length(min = 1, max = 20, message = "Name must be 1-20 characters"))]
    pub name: String,
    pub can_size: f64,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LeaveBeerTrackerRequest {
    #[validate(length(min = 1, max = 20, message = "Name must be 1-20 characters"))]
    pub name: String,
}
