mod r#type;
mod action_type;

use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct ContentRefresh{
    pub content_type: r#type::Type,
    pub action_type: action_type::ActionType,
}
