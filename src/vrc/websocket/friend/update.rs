use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct FriendUpdate{
    pub user_id: Arc<str>,
    pub user: super::User,
}