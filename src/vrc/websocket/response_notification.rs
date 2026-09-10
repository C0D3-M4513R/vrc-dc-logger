use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct ResponseNotification{
    pub notification_id: Arc<str>,
    pub receiver_id: Arc<str>,
    pub response_id: Arc<str>,
}
