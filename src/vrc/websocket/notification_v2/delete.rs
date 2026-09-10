use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[serde(rename_all="camelCase")]
pub struct NotificationV2Delete{
    ///Notification id
    pub ids: Arc<[Arc<str>]>,
    pub version: i32,
}