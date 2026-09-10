use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct UserBadgeAssigned{
    pub badge: vrchatapi::models::badge::Badge
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct UserBadgeUnassigned{
    pub badge_id: Arc<str>
}
