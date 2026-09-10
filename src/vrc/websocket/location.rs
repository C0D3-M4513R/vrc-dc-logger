use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[serde(from = "&str", into = "String")]
pub enum Location {
    Offline,
    Traveling,
    Private,
    Null,
    ///should be a world id (or maybe an undocumented variant)
    Other(Arc<str>)
}

impl From<&str> for Location {
    fn from(value: &str) -> Self {
        match value {
            "offline" => Self::Offline,
            "traveling" => Self::Traveling,
            "private" => Self::Private,
            "" => Self::Null,
            other => Self::Other(Arc::from(other)),
        }
    }
}
impl From<Location> for String{
    fn from(value: Location) -> Self {
        match value {
            Location::Offline => "offline".to_string(),
            Location::Traveling => "traveling".to_string(),
            Location::Private => "private".to_string(),
            Location::Null => "".to_string(),
            Location::Other(other) => other.to_string(),
        }
    }
}

#[cfg(test)]
mod test{
    use std::sync::Arc;

    #[test]
    fn test_serialise()->Result<(), crate::Error>{
        for (string, expected) in [
            (r#""offline""#, super::Location::Offline),
            (r#""traveling""#, super::Location::Traveling),
            (r#""private""#, super::Location::Private),
            (r#""""#, super::Location::Null),
            (r#""ios""#, super::Location::Other(Arc::from("ios"))),
            (r#""other""#, super::Location::Other(Arc::from("other"))),
        ] {
            println!("serialising {expected:?}");
            assert_eq!(serde_json::to_string(&expected)?, string.to_string());
            println!("deserialising {string}");
            assert_eq!(serde_json::from_str::<super::Location>(string)?, expected)
        }
        Ok(())
    }
}