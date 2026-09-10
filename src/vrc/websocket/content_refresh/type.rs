use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[serde(from = "&str", into = "String")]
pub enum Type {
    Gallery,
    Icon,
    Emoji,
    Avatar,
    World,
    Other(Arc<str>)
}

impl From<&str> for Type {
    fn from(value: &str) -> Self {
        match value {
            "gallery" => Self::Gallery,
            "icon" => Self::Icon,
            "emoji" => Self::Emoji,
            "avatar" => Self::Avatar,
            "world" => Self::World,
            other => Self::Other(Arc::from(other)),
        }
    }
}
impl From<Type> for String{
    fn from(value: Type) -> Self {
        match value {
            Type::Gallery => "gallery".to_string(),
            Type::Icon => "icon".to_string(),
            Type::Emoji => "emoji".to_string(),
            Type::Avatar => "avatar".to_string(),
            Type::World => "world".to_string(),
            Type::Other(other) => other.to_string(),
        }
    }
}

#[cfg(test)]
mod test{
    use std::sync::Arc;

    #[test]
    fn test_serialise()->Result<(), crate::Error>{
        for (string, expected) in [
            (r#""gallery""#, super::Type::Gallery),
            (r#""icon""#, super::Type::Icon),
            (r#""emoji""#, super::Type::Emoji),
            (r#""avatar""#, super::Type::Avatar),
            (r#""world""#, super::Type::World),
            (r#""ios""#, super::Type::Other(Arc::from("ios"))),
            (r#""other""#, super::Type::Other(Arc::from("other"))),
        ] {
            println!("serialising {expected:?}");
            assert_eq!(serde_json::to_string(&expected)?, string.to_string());
            println!("deserialising {string}");
            assert_eq!(serde_json::from_str::<super::Type>(string)?, expected)
        }
        Ok(())
    }
}