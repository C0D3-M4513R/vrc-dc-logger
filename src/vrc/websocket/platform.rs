use std::fmt::{Display, Formatter};
use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[serde(from = "&str", into = "String")]
pub enum Platform{
    StandaloneWindows,
    Android,
    Web,
    Null,
    Other(Arc<str>)
}

impl From<&str> for Platform{
    fn from(value: &str) -> Self {
        match value {
            "standalonewindows" => Self::StandaloneWindows,
            "android" => Self::Android,
            "web" => Self::Web,
            "" => Self::Null,
            other => Self::Other(Arc::from(other)),
        }
    }
}
impl From<Platform> for String {
    fn from(value: Platform) -> Self {
        Into::<&str>::into(&value).to_string()
    }
}
impl<'a> From<&'a Platform> for &'a str{
    fn from(value: &'a Platform) -> Self {
        match value {
            Platform::StandaloneWindows => "standalonewindows",
            Platform::Android => "android",
            Platform::Web => "web",
            Platform::Null => "",
            Platform::Other(other) => other,
        }
    }
}

impl Display for Platform {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Into::<&str>::into(self).fmt(f)
    }
}

#[cfg(test)]
mod test{
    use std::sync::Arc;

    #[test]
    fn test_serialise()->Result<(), crate::Error>{
        for (string, expected) in [
            (r#""standalonewindows""#, super::Platform::StandaloneWindows),
            (r#""android""#, super::Platform::Android),
            (r#""web""#, super::Platform::Web),
            (r#""""#, super::Platform::Null),
            (r#""ios""#, super::Platform::Other(Arc::from("ios"))),
            (r#""other""#, super::Platform::Other(Arc::from("other"))),
        ] {
            println!("serialising {expected:?}");
            assert_eq!(serde_json::to_string(&expected)?, string.to_string());
            println!("deserialising {string}");
            assert_eq!(serde_json::from_str::<super::Platform>(string)?, expected)
        }
        Ok(())
    }
}