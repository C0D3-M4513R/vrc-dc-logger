use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct InstanceQueueJoined{
    pub instance_location: super::location::Location,
    pub position: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct InstanceQueueReady{
    pub instance_location: super::location::Location,
    pub expiry: chrono::DateTime<chrono::Utc>,
}
