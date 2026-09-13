use std::sync::Arc;
use thiserror::Error;
use vrchatapi::models::UserState;
use crate::vrc::websocket::Message;

#[derive(Debug, Error)]
pub enum WSElementError{
    #[error("SerdeJson: {error}")]
    SerdeJson{error: serde_json::Error, message: tokio_websockets::proto::Message},
    #[cfg(not(target_family = "wasm"))]
    #[error("UnsupportedMessageType: {0:?}")]
    UnsupportedMessageType(tokio_websockets::proto::Message),
    #[cfg(not(target_family = "wasm"))]
    #[error("WebSocketError: {0}")]
    WebSocketError(tokio_websockets::Error),
}

#[derive(Debug, Error)]
pub enum StartWSError{
    #[error("Error from Http crate: {0}")]
    Http(#[from] http::Error),
    #[cfg(not(target_family = "wasm"))]
    #[error("Error from Tokio_Websockets: {0}")]
    TokioWebsockets(#[from] tokio_websockets::Error),
    #[cfg(target_family = "wasm")]
    #[error("Error from web_sys: {0}")]
    WebSys(String),
}
pub trait WSHandler{
    fn handler(&mut self, message:Result<Message, WSElementError>);
}

#[cfg(not(target_family = "wasm"))]
mod tokio_websocket{
    #![cfg(not(target_family = "wasm"))]
    use core::ops::Fn;
    use core::task::Context;
    use core::pin::Pin;
    use core::task::Poll;
    use std::sync::Arc;
    use std::time::Duration;
    use futures::StreamExt;
    use vrchatapi::models::UserState;
    use crate::vrc::websocket::connection::WSElementError;
    use crate::vrc::websocket::Message;

    pub struct TokioWebspocket {
        ws: tokio_websockets::WebSocketStream<tokio_websockets::MaybeTlsStream<tokio::net::TcpStream>>,
    }

    impl TokioWebspocket {
        pub async fn new<T: super::WSHandler + Send + 'static>(
            auth_token: &str,
            cookies: http::HeaderValue,
            mut storage: T
        ) -> Result<tokio::sync::oneshot::Sender<()>, super::StartWSError>
        {
            let (sender, mut reciever) = tokio::sync::oneshot::channel();
            let url = http::Uri::builder()
                .scheme("wss")
                .authority(super::super::super::DOMAIN)
                .path_and_query(format!("/?authToken={auth_token}"))
                .build()?;

            let ws_builder = tokio_websockets::client::Builder::from_uri(url)
                .add_header(http::header::COOKIE, cookies)?
                .add_header(http::header::USER_AGENT, http::HeaderValue::from_static(super::super::super::VRC_USER_AGENT))?
                ;
            let mut this = Self{
                ws: ws_builder.connect().await?.0,
            };

            crate::spawn(async move{
                let mut closed = false;
                let mut timeout = false;
                let mut recieved_message = false;
                let mut failed_reconnect_count = 0;
                loop {
                    tokio::select! {
                    biased;
                    _ = &mut reciever => {
                        log::info!("Websocket termination requested. Closing.");
                        break
                    }
                    _ = tokio::time::sleep(Duration::from_secs(30)), if timeout => {
                        log::warn!("Websocket reconnection timeout of 30s passed.");
                        timeout = false;
                    },
                    ws = ws_builder.connect(), if closed => {
                        match ws {
                            Ok((ws, _)) => {
                                this.ws = ws;
                                closed = false;
                            },
                            Err(err) => {
                                failed_reconnect_count += 1;
                                timeout = true;
                                log::warn!("Failed to reconnect to WebSocket: {err}");
                                if failed_reconnect_count > 10 {
                                    log::error!("Too many failed WebSocket connection attempts. Exiting Websocket handler");
                                    break
                                }
                            }
                        }
                    },
                    next = this.next(), if !closed => {
                        match next {
                            Some(v) => storage.handler(v),
                            None => {
                                log::info!("Websocket closed. Trying to reconnect.");
                                closed = true;
                            },
                        }
                    },

                }
                }
            });

            Ok(sender)
        }
    }
    impl futures::Stream for TokioWebspocket {
        type Item = Result<super::Message, super::WSElementError>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let mut pinned = std::pin::pin!(&mut self.ws);
            match pinned.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(message))) => {
                    if let Some((close, reason)) = message.as_close() {
                        log::info!("Websocket Closed with code {close:?} and reason: {reason}");
                        Poll::Ready(None)
                    } else {
                        let message_content = &**message.as_payload();
                        if message_content.is_empty(){
                            log::debug!("Recieved empty Websocket Message. Likely to KeepAlive?");
                            Poll::Pending
                        }else{
                            match serde_json::from_slice(message_content) {
                                Ok(message) => Poll::Ready(Some(Ok(message))),
                                Err(err) => {
                                    if let Some(text) = message.as_text(){
                                        log::error!("Failed to decode Websocket message: error:{err}, message: {text}", );
                                    }else{
                                        log::error!("Failed to decode Websocket message: error:{err}, message: {message_content:#?}");
                                    }
                                    Poll::Ready(Some(Err(WSElementError::SerdeJson{error: err, message})))
                                },
                            }
                        }
                    }
                },
                Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(WSElementError::WebSocketError(err)))),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending
            }
        }
    }
}

#[cfg(not(target_family = "wasm"))]
pub use tokio_websocket::TokioWebspocket as WebSocket;
