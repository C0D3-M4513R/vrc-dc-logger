use serde::{Deserialize, Serialize};
use crate::vrc::websocket::location::Location;
#[derive(Debug, Deserialize, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[serde(from = "&str", into="String")]
pub enum NotificationV2ResponseIcon {
    Check,
    BellSlash,
    Other(String)
}
impl From<&str> for NotificationV2ResponseIcon {
    fn from(value: &str) -> Self {
        match value {
            "check" => Self::Check,
            "bell-slash" => Self::BellSlash,
            other => Self::Other(other.to_string()),
        }
    }
}
impl From<NotificationV2ResponseIcon> for String{
    fn from(value: NotificationV2ResponseIcon) -> Self {
        match value {
            NotificationV2ResponseIcon::Check => "check".to_string(),
            NotificationV2ResponseIcon::BellSlash => "bell-slash".to_string(),
            NotificationV2ResponseIcon::Other(other) => other,
        }
    }
}

#[cfg(test)]
mod test{
    #[test]
    fn test_serialise()->Result<(), crate::Error>{
        for (string, expected) in [
            (r#""check""#, super::NotificationV2ResponseIcon::Check),
            (r#""bell-slash""#, super::NotificationV2ResponseIcon::BellSlash),
            (r#""ios""#, super::NotificationV2ResponseIcon::Other("ios".to_string())),
            (r#""other""#, super::NotificationV2ResponseIcon::Other("other".to_string())),
        ] {
            println!("serialising {expected:?}");
            assert_eq!(serde_json::to_string(&expected)?, string.to_string());
            println!("deserialising {string}");
            assert_eq!(serde_json::from_str::<super::NotificationV2ResponseIcon>(string)?, expected)
        }
        Ok(())
    }
}