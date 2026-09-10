use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct FriendOnline{
    pub user_id: Arc<str>,
    pub platform: super::super::platform::Platform,
    pub location: super::super::location::Location,
    pub can_request_invite: bool,
    pub user: super::User,
}
