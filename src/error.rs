
#[derive(Debug)]
pub enum VRCError<T: std::any::Any + Sized> {
    Error(vrchatapi::apis::Error<T>),
}
impl<T: std::any::Any + Sized> From<vrchatapi::apis::Error<T>> for VRCError<T> {
    fn from(e: vrchatapi::apis::Error<T>) -> Self {
        VRCError::Error(e)
    }
}
impl<T: std::any::Any + Sized> Display for VRCError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match match self {
            VRCError::Error(e) => e,
        } {
            vrchatapi::apis::Error::Reqwest(e) => {
                write!(f, "vrchatapi::apis::Error::Reqwest({})", e)
            }
            vrchatapi::apis::Error::ReqwestMiddleware(e) => {
                write!(f, "vrchatapi::apis::Error::ReqwestMiddleware({})", e)
            }
            vrchatapi::apis::Error::Serde(e) => write!(f, "vrchatapi::apis::Error::Serde({})", e),
            vrchatapi::apis::Error::Io(e) => write!(f, "vrchatapi::apis::Error::Io({})", e),
            vrchatapi::apis::Error::ResponseError(e) => write!(
                f,
                "vrchatapi::apis::Error::ResponseError(status:{}, content: {}, entity:{})",
                e.status,
                e.content,
                if e.entity.is_some() { "Some" } else { "None" }
            ),
        }
    }
}

impl<T: std::any::Any + Debug + Sized> std::error::Error for VRCError<T> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match match self {
            VRCError::Error(e) => e,
        } {
            vrchatapi::apis::Error::Reqwest(e) => Some(e),
            vrchatapi::apis::Error::ReqwestMiddleware(e) => Some(e),
            vrchatapi::apis::Error::Serde(e) => Some(e),
            vrchatapi::apis::Error::Io(e) => Some(e),
            vrchatapi::apis::Error::ResponseError(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Reqwest(reqwest::Error),
    Vrchat(VRCError<()>),
    LoginError(crate::vrc::LoginError<()>),
    SerdeJsonError(serde_json::error::Error),
    Boxed(Box<dyn std::error::Error + Send + Sync>),
}
macro_rules! impl_from {
            ($name:path$([$($typeVar:ident$(:$typeBound:path)*),+])?, map, $from:ty,  $($(#[$itemAttr:meta])* $item:pat $(if $itemIf:expr)? => $itemTo:expr$(,)?)+) => {
                impl$(<$($typeVar$(:$typeBound)*),+>)? From<$from> for $name {
                    fn from(e: $from) -> Self {
                        match e {
                            $(
                                $(#[$itemAttr])* $item $(if $itemIf)? => $itemTo
                            ),+
                        }
                    }
                }
            };
            ($name:path$([$($typeVar:ident$(:$typeBound:path)*),+])?, delegate, $paramName:ident:$from:ty, $delegate:expr) => {
                impl$(<$($typeVar$(:$typeBound)*),+>)? From<$from> for $name {
                    fn from($paramName: $from) -> Self {
                        $delegate
                    }
                }
            };
            ($name:path$([$($typeVar:ident$(:$typeBound:path)*),+])?, $from:ty, $to:ident) => {
                impl$(<$($typeVar$(:$typeBound)*),+>)? From<$from> for $name {
                    fn from(e: $from) -> Self {
                        <$name>::$to(e)
                    }
                }
            };
        }
use std::fmt::{Debug, Display, Formatter};
pub(crate) use impl_from;
impl_from!(Error, std::io::Error, Io);
impl_from!(Error, VRCError<()>, Vrchat);
impl_from!(
    Error,
    delegate,
    e: vrchatapi::apis::Error<()>,
    Error::Vrchat(VRCError::Error(e))
); //yes, this duplicates cases, but it might be useful to know what library an error was from.
impl_from!(Error, reqwest::Error, Reqwest);
impl_from!(Error, serde_json::error::Error, SerdeJsonError);
impl_from!(Error, Box<dyn std::error::Error + Send + Sync>, Boxed);

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "Error::Io({})", e),
            Error::Reqwest(e) => write!(f, "Error::Reqwest({})", e),
            Error::Vrchat(e) => write!(f, "Error::Vrchat({})", e),
            Error::LoginError(e) => write!(f, "Error::LoginError({})", e),
            Error::SerdeJsonError(e) => write!(f, "Error::SerdeJsonError({})", e),
            // Error::FileUtils(e) => write!(f, "Error::FileUtils({})", e),
            Error::Boxed(e) => write!(f, "Error::Boxed({})", e),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Reqwest(e) => Some(e),
            Error::Vrchat(e) => Some(e),
            Error::LoginError(e) => Some(e),
            Error::SerdeJsonError(e) => Some(e),
            Error::Boxed(e) => Some(e.as_ref()),
        }
    }
}
