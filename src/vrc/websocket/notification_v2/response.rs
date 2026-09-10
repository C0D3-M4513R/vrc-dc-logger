use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::vrc::websocket::notification_v2::{r#type, resonse_icon};

#[derive(Debug, Deserialize, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct NotificationV2Response{
    pub r#type: r#type::NotificationV2Type,
    pub data: Arc<str>,
    pub icon: resonse_icon::NotificationV2ResponseIcon,
    pub text: Arc<str>,
}