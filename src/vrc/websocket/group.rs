use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct GroupJoined{
    pub group_id: Arc<str>
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct GroupLeft {
    pub group_id: Arc<str>
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct GroupMemberUpdated {
    pub member: vrchatapi::models::group_member_limited_user::GroupMemberLimitedUser
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct GroupRoleUpdated {
    pub member: vrchatapi::models::group_role::GroupRole
}