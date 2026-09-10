use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct NotificationV2Update{
    ///Notification id
    pub id: Arc<str>,
    pub version: i32,
    pub updates: NotificationV2Updates,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[serde(rename_all="camelCase", default)]
pub struct NotificationV2Updates{
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<super::super::platform::Platform>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<super::r#type::NotificationV2Type>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<super::category::NotificationV2Category>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_system: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_d_n_d: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_user_id: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_username: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver_user_id: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_notifications_id: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<Arc<str>>, //todo: handle path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_text: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responses: Option<Arc<[super::response::NotificationV2Response]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_after_seen: Option<isize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_seen: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}