use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct FriendActive{
    pub user_id: Arc<str>,
    pub platform: super::super::platform::Platform,
    pub user: super::User
}
