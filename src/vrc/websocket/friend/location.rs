use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct FriendLocation {
    pub user_id: Arc<str>,
    pub location: super::super::location::Location,
    /// normally empty "", but when the above "location" is "traveling", this contains the imminent destination
    pub traveling_to_location: super::super::location::Location,
    pub world_id: Arc<str>,
    pub can_request_invite: bool,
    pub user: super::User,
}
