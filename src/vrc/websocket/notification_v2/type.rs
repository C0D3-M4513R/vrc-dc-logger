use serde::{Deserialize, Serialize};
use crate::vrc::websocket::location::Location;
#[derive(Debug, Deserialize, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[serde(from = "&str", into="String")]
pub enum NotificationV2Type {
    GroupAnnouncement,
    Other(String)
}
impl From<&str> for NotificationV2Type {
    fn from(value: &str) -> Self {
        match value {
            "group.announcement" => Self::GroupAnnouncement,
            other => Self::Other(other.to_string()),
        }
    }
}
impl From<NotificationV2Type> for String{
    fn from(value: NotificationV2Type) -> Self {
        match value {
            NotificationV2Type::GroupAnnouncement => "group.announcement".to_string(),
            NotificationV2Type::Other(other) => other,
        }
    }
}

#[cfg(test)]
mod test{
    #[test]
    fn test_serialise()->Result<(), crate::Error>{
        for (string, expected) in [
            (r#""group.announcement""#, super::NotificationV2Type::GroupAnnouncement),
            (r#""ios""#, super::NotificationV2Type::Other("ios".to_string())),
            (r#""other""#, super::NotificationV2Type::Other("other".to_string())),
        ] {
            println!("serialising {expected:?}");
            assert_eq!(serde_json::to_string(&expected)?, string.to_string());
            println!("deserialising {string}");
            assert_eq!(serde_json::from_str::<super::NotificationV2Type>(string)?, expected)
        }
        Ok(())
    }
}