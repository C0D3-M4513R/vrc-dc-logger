use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[serde(try_from = "&str", into = "&str")]
pub enum ActionType {
    Created,
    Deleted,
}

impl<'a> TryFrom<&'a str> for ActionType {
    type Error = &'a str;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        match value {
            "created" => Ok(Self::Created),
            "deleted" => Ok(Self::Deleted),
            other => Err(other),
        }
    }
}
impl From<ActionType> for &'static str{
    fn from(value: ActionType) -> Self {
        match value {
            ActionType::Created => "created",
            ActionType::Deleted => "deleted",
        }
    }
}

#[cfg(test)]
mod test{
    use std::sync::Arc;

    #[test]
    fn test_serialise()->Result<(), crate::Error>{
        for (string, expected) in [
            (r#""created""#, super::ActionType::Created),
            (r#""deleted""#, super::ActionType::Deleted),
        ] {
            println!("serialising {expected:?}");
            assert_eq!(serde_json::to_string(&expected)?, string.to_string());
            println!("deserialising {string}");
            assert_eq!(serde_json::from_str::<super::ActionType>(string)?, expected)
        }
        Ok(())
    }
}