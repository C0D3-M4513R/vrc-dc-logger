use serde::{Deserialize, Serialize};
use crate::vrc::websocket::location::Location;
#[derive(Debug, Deserialize, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[serde(from = "&str", into="String")]
pub enum NotificationV2Category {
    SocialGroup,
    Other(String)
}
impl From<&str> for NotificationV2Category {
    fn from(value: &str) -> Self {
        match value {
            "social.group" => Self::SocialGroup,
            other => Self::Other(other.to_string()),
        }
    }
}
impl From<NotificationV2Category> for String{
    fn from(value: NotificationV2Category) -> Self {
        match value {
            NotificationV2Category::SocialGroup => "social.group".to_string(),
            NotificationV2Category::Other(other) => other,
        }
    }
}

#[cfg(test)]
mod test{
    #[test]
    fn test_serialise()->Result<(), crate::Error>{
        for (string, expected) in [
            (r#""social.group""#, super::NotificationV2Category::SocialGroup),
            (r#""ios""#, super::NotificationV2Category::Other("ios".to_string())),
            (r#""other""#, super::NotificationV2Category::Other("other".to_string())),
        ] {
            println!("serialising {expected:?}");
            assert_eq!(serde_json::to_string(&expected)?, string.to_string());
            println!("deserialising {string}");
            assert_eq!(serde_json::from_str::<super::NotificationV2Category>(string)?, expected)
        }
        Ok(())
    }
}