use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all="camelCase")]
pub struct UserLocation{
    pub user_id: Arc<str>,
    pub user: vrchatapi::models::current_user::CurrentUser,
    pub location: super::super::location::Location,
    /// This is location without the World ID part.
    pub instance: Arc<str>,
    pub world_id: Arc<str>,
    pub world: vrchatapi::models::world::World,
}