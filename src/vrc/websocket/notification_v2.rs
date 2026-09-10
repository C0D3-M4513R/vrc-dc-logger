pub mod r#type;
pub mod category;
pub mod resonse_icon;
pub mod update;
pub mod response;
pub mod delete;

use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[serde(rename_all="camelCase")]
pub struct NotificationV2{
    ///Notification id
    pub id: Arc<str>,
    pub version: i32,
    pub platform: super::platform::Platform,
    pub r#type: r#type::NotificationV2Type,
    pub category: category::NotificationV2Category,
    pub is_system: bool,
    pub ignore_d_n_d: bool,
    pub sender_user_id: Arc<str>,
    pub sender_username: Arc<str>,
    pub receiver_user_id: Arc<str>,
    pub related_notifications_id: Arc<str>,
    pub title: Arc<str>,
    pub message: Arc<str>,
    pub image_url: Arc<str>,
    pub link: Arc<str>, //todo: handle path
    pub link_text: Arc<str>,
    pub responses: Arc<[response::NotificationV2Response]>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub expiry_after_seen: isize,
    pub require_seen: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}