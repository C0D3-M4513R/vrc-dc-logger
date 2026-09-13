#![forbid(unsafe_code)]
mod vrc;
mod error;

use std::sync::Arc;
use tokio::sync::Mutex;
pub(crate) use error::{VRCError, Error, impl_from};

pub use tokio::task::spawn as spawn;
pub use tokio::task::spawn_local as spawn_local;
pub use tokio::task::spawn_blocking as spawn_blocking;
use vrchatapi::models::RegisterUserAccount200Response;
use crate::vrc::LoginError;
use crate::vrc::websocket::connection::WSElementError;
use crate::vrc::websocket::Message;

fn main() -> anyhow::Result<()> {
    match dotenvy::dotenv(){
        Ok(_) => {},
        Err(err) if err.not_found() => {},
        Err(err) => {
            eprintln!("Error loading .env: {err}");
            return Err(err.into())
        }
    }
    simple_logger::SimpleLogger::new()
        .with_utc_timestamps()
        .with_colors(true)
        .with_level(log::LevelFilter::Info)
        .with_module_level("eframe", log::LevelFilter::Info)
        .env()
        .init()
        .expect("Failed to initialize logger");
    log::info!("Logger initialized");
    async_main()
}

#[derive(Debug, serde_derive::Deserialize)]
struct ConfigFile{
    configs: Box<[VRCConfig]>,
    owner: serenity::model::id::UserId,
}

#[derive(Debug, serde_derive::Deserialize)]
struct VRCConfig{
    vrchat_cookies: Box<[cookie_store::Cookie<'static>]>,
    config: Config,
}

#[derive(Debug, serde_derive::Deserialize)]
struct Config{
    tracking_users: std::collections::HashMap<Box<str>, ConfigUser>,
}

#[derive(Debug, serde_derive::Deserialize)]
struct ConfigUser {
    pub forward_ids: Box<[serenity::model::id::UserId]>,
    pub backup_name: Box<str>,
}

struct Handler {
    config: Config,
    owner: serenity::model::id::UserId,
    dc: CacheHttp,
    vrc: Arc<vrc::Vrc>,
}
#[derive(Debug, Clone)]
struct CacheHttp {
    http: Arc<serenity::http::Http>,
    cache: Arc<serenity::cache::Cache>,
}
impl serenity::prelude::CacheHttp for CacheHttp {
    fn http(&self) -> &serenity::http::Http {
        &*self.http
    }

    fn cache(&self) -> Option<&Arc<serenity::cache::Cache>> {
        Some(&self.cache)
    }
}

fn get_url(user: &vrchatapi::models::User) -> &str {
    user.icon_url
        .as_ref()
        .or_else(||{ let url = &user.profile_pic_override_thumbnail; if !url.is_empty() {Some(url)} else {None} })
        .unwrap_or(&user.current_avatar_thumbnail_image_url)
}

impl vrc::websocket::connection::WSHandler for tokio::sync::OwnedMutexGuard<Handler> {
    fn handler(&mut self, message: Result<Message, WSElementError>) {
        log::info!("Message: {message:?}");
        macro_rules! handle_msg {
            ($uid:expr, $user:expr, $action_name:literal, $($element:expr),*) => {
                if let Some(user) = self.config.tracking_users.get($uid) {
                    let vrc_user = &$user;
                    let body:serenity::builder::CreateMessage =
                        serenity::builder::CreateMessage::new()
                        .flags(serenity::model::channel::MessageFlags::IS_COMPONENTS_V2)
                        .components(vec![
                            serenity::builder::CreateComponent::Container(serenity::builder::CreateContainer::new(vec![
                                serenity::builder::CreateContainerComponent::Section(serenity::builder::CreateSection::new(
                                    vec![
                                        serenity::builder::CreateSectionComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("# {} - {} - {}", $action_name, user.backup_name, vrc_user.display_name))),
                                        serenity::builder::CreateSectionComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("State: {}, Status: {}, Description: {}", vrc_user.state, vrc_user.status, vrc_user.status_description))),
                                        serenity::builder::CreateSectionComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Bio:\n {}", vrc_user.bio)))
                                    ],
                                    serenity::builder::CreateSectionAccessory::Thumbnail(serenity::builder::CreateThumbnail::new(
                                        serenity::builder::CreateUnfurledMediaItem::new(get_url(vrc_user).to_string())
                                    ).description(format!("Profile Image of - {}", vrc_user.display_name))))
                                ),
                                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Bio Links: {}", vrc_user.bio_links.iter().map(|v|format!("\n 1. {v}")).fold(String::new(), |mut a, b|{a.push_str(&b); a})))),
                                $($element),*
                            ]))
                        ]);
                    for id in user.forward_ids.iter().copied() {
                        let body = body.clone();
                        let dc = self.dc.clone();
                        tokio::spawn(async move {
                            if let Err(err) = id.direct_message(dc, body).await {
                                log::error!("Failed to send message to {id}: {err}")
                            }
                        });
                    }
                }
            };
            ($uid:expr, $action_name:literal, $($element:expr),*) => {
                if let Some(user) = self.config.tracking_users.get($uid) {
                    let body:serenity::builder::CreateMessage =
                        serenity::builder::CreateMessage::new()
                            .flags(serenity::model::channel::MessageFlags::IS_COMPONENTS_V2)
                            .components(vec![
                                serenity::builder::CreateComponent::Container(serenity::builder::CreateContainer::new(vec![
                                    serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("# {} - {}", $action_name, user.backup_name))),
                                    $($element),*
                                ]))
                            ]);
                    for id in user.forward_ids.iter().copied() {
                        let body = body.clone();
                        let dc = self.dc.clone();
                        tokio::spawn(async move {
                            if let Err(err) = id.direct_message(dc, body).await {
                                log::error!("Failed to send message to {id}: {err}")
                            }
                        });
                    }
                }
            };
        }
        match message {
            Ok(Message::FriendAdd { content }) => handle_msg!(&*content.user_id, content.user, "FriendAdd",),
            Ok(Message::FriendDelete { content }) => handle_msg!(&*content.user_id, "FriendDelete",),
            Ok(Message::FriendOnline { content }) => handle_msg!(&*content.user_id, content.user, "FriendOnline",
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Platform: {}", content.platform))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Location: {}", content.location))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Can Request Invite: {}", content.can_request_invite)))
            ),
            Ok(Message::FriendActive { content }) => handle_msg!(&*content.user_id, content.user, "FriendActive",
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Platform: {}", content.platform)))
            ),
            Ok(Message::FriendOffline { content }) => handle_msg!(&*content.user_id, "FriendOffline",
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Platform: {}", content.platform)))
            ),
            Ok(Message::FriendUpdate { content }) => handle_msg!(&*content.user_id, content.user, "FriendUpdate",),
            Ok(Message::FriendLocation { content }) => handle_msg!(&*content.user_id, content.user, "FriendLocation",
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Location: {}", content.location))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Traveling to Location: {}", content.traveling_to_location))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("World Id: {}", content.world_id))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Can Request Invite: {}", content.can_request_invite)))
            ),
            Ok(Message::UserUpdate { .. }) => {}
            Ok(Message::UserLocation { .. }) => {}
            Ok(Message::UserBadgeAssigned { .. }) => {}
            Ok(Message::UserBadgeUnassigned { .. }) => {}
            Ok(Message::Notification { .. })  => {}
            Ok(Message::ResponseNotification { .. })  => {}
            Ok(Message::SeeNotification { .. })  => {}
            Ok(Message::HideNotification { .. })  => {}
            Ok(Message::ClearNotification)  => {}
            Ok(Message::NotificationV2 { .. })  => {}
            Ok(Message::NotificationV2Update { .. })  => {}
            Ok(Message::NotificationV2Delete { .. })  => {}
            Ok(Message::ContentRefresh { .. })  => {}
            Ok(Message::InstanceQueueJoined { .. })  => {}
            Ok(Message::InstanceQueueReady { .. })  => {}
            Ok(Message::GroupJoined { .. })  => {}
            Ok(Message::GroupLeft { .. })  => {}
            Ok(Message::GroupMemberUpdated { .. })  => {}
            Ok(Message::GroupRoleUpdated { .. })  => {}
            Err(WSElementError::SerdeJson {error, message}) => {
                let dc = self.dc.clone();
                let id = self.owner;
                let body = serenity::builder::CreateMessage::new()
                    .flags(serenity::model::channel::MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![
                        serenity::builder::CreateComponent::Container(serenity::builder::CreateContainer::new(vec![
                            serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new("# Failed to Deserialize Websocket Message")),
                            serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Error: `{error}`"))),
                            serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Data: ```\n{}\n```", String::from_utf8_lossy(&message.as_payload())))),
                        ]))
                    ]);
                tokio::task::spawn(async move {
                    if let Err(err) = id.direct_message(dc, body).await {
                        log::error!("Failed to send message to {id}: {err}")
                    }
                });
                log::error!("failed receiving ws element: {error}")
            },
            Err(err) => {
                log::error!("failed receiving ws element: {err}")
            },
        }
    }
}

#[tokio::main]
async fn async_main() -> anyhow::Result<()> {
    let config: ConfigFile = serde_json::from_slice(&tokio::fs::read("config.json").await?)?;
    let ConfigFile{configs, owner} = config;
    let mut settings = serenity::cache::Settings::default();
    settings.cache_guilds = false;
    settings.max_messages = 0;
    settings.cache_users = true;
    let mut client = serenity::Client::builder(serenity::all::Token::from_env("DISCORD_TOKEN")?, serenity::prelude::GatewayIntents::empty())
        .cache_settings(settings)
        .await?
    ;

    {
        let shutdown = client.shard_manager.get_shutdown_trigger();
        tokio::task::spawn(async{
            tokio::signal::ctrl_c().await.unwrap();
            shutdown();
        });
    }

    for config in configs {
        let VRCConfig{vrchat_cookies, config} = config;
        let vrc = Arc::new(vrc::Vrc::new(vrchat_cookies.into_iter())?);
        let handler = Arc::new(Mutex::new(Handler {
            config,
            owner,
            dc: CacheHttp {
                http: client.http.clone(),
                cache: client.cache.clone(),
            },
            vrc: vrc.clone(),
        }));
        match vrc.check_auth(handler.lock_owned().await).await.expect("VRChat isn't logged in") {
            RegisterUserAccount200Response::CurrentUser(_) => {}
            RegisterUserAccount200Response::RequiresTwoFactorAuth(_) => {
                eprintln!("VRChat wants 2fa");
                return Err(Error::LoginError(LoginError::Needs2fa).into());
            }
        }
    }


    client.start_autosharded().await?;
    Ok(())
}