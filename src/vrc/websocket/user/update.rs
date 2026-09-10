use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct UserUpdate{
    pub user_id: Arc<str>,
    pub user: UserUpdateUser
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct UserUpdateUser {
    pub bio: Arc<str>,
    pub current_avatar: Arc<str>,
    pub current_avatar_asset_url: Arc<str>,
    pub current_avatar_image_url: Arc<str>,
    pub display_name: Arc<str>,
    pub fallback_avatar: Arc<str>,
    pub id: Arc<str>,
    pub profile_pic_override: Arc<str>,
    pub status: vrchatapi::models::user_status::UserStatus,
    pub status_description: Arc<str>,
    pub tags: Arc<[Arc<str>]>,
    pub user_icon: Arc<str>,
    #[serde(default)]
    pub username: Option<Arc<str>>,
}