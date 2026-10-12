#![forbid(unsafe_code)]
mod vrc;
mod error;

use std::borrow::Cow;
use std::sync::Arc;
use serenity::all::{Context, FullEvent, Interaction};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
pub(crate) use error::{VRCError, Error, impl_from};

pub use tokio::task::spawn as spawn;
pub use tokio::task::spawn_local as spawn_local;
pub use tokio::task::spawn_blocking as spawn_blocking;
use vrchatapi::models::{CurrentUserLoginResponse, WebsocketMessage};
use crate::vrc::LoginError;
use crate::vrc::websocket::WSElementError;

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
        .with_module_level("vrchatapi", log::LevelFilter::Debug)
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

#[derive(Debug, Clone, serde_derive::Deserialize)]
struct Config{
    tracking_users: Arc<std::collections::HashMap<Box<str>, ConfigUser>>,
    vrc_user_id: Arc<str>,
    account_owners: Arc<std::collections::HashSet<serenity::model::id::UserId>>,
}

#[derive(Debug, Clone, serde_derive::Deserialize)]
struct ConfigUser {
    pub forward_ids: Box<std::collections::HashSet<serenity::model::id::UserId>>,
    pub backup_name: Box<str>,
}

struct Handler {
    ws_log_file: Arc<Mutex<tokio::fs::File>>,
    i: usize,
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

impl vrc::websocket::WSHandler for tokio::sync::OwnedMutexGuard<Handler> {
    fn handler(&mut self, message_arg: Result<(tokio_websockets::proto::Message, WebsocketMessage), WSElementError>) {
        log::info!("Message: {message_arg:?}");
        macro_rules! handle_msg {
            ($uid:expr, $user:expr, $action_name:literal, $($element:expr),* $(; $additional:expr),*) => {
                if let Some(user) = self.config.tracking_users.get($uid) {
                    let vrc_user = &$user;
                    let container = vec![
                        serenity::builder::CreateContainerComponent::Section(serenity::builder::CreateSection::new(
                            vec![
                                serenity::builder::CreateSectionComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("# {} - {} - {}", $action_name, user.backup_name, vrc_user.display_name))),
                                serenity::builder::CreateSectionComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("State: {}, Status: {}, Description: {}", vrc_user.state, vrc_user.status, vrc_user.status_description))),
                            ],
                            serenity::builder::CreateSectionAccessory::Thumbnail(serenity::builder::CreateThumbnail::new(
                                serenity::builder::CreateUnfurledMediaItem::new(
                                    vrc_user.icon_url
                                    .as_ref()
                                    .cloned()
                                    .map(Cow::Owned)
                                    .unwrap_or(Cow::Borrowed("https://cdn.discordapp.com/embed/avatars/1.png")))
                            ).description(format!("Profile Image of - {}", vrc_user.display_name))))
                        ),
                        $($element),*
                    ];
                    $(let container = {
                        let mut container = container;
                        container.extend($additional);
                        container
                    };)*

                    let body:serenity::builder::CreateMessage =
                        serenity::builder::CreateMessage::new()
                        .flags(serenity::model::channel::MessageFlags::IS_COMPONENTS_V2)
                        .components(vec![
                            serenity::builder::CreateComponent::Container(serenity::builder::CreateContainer::new(container))
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
            ($uid:expr, $action_name:literal, $($element:expr),* $(; $additional:expr),*) => {
                if let Some(user) = self.config.tracking_users.get($uid) {
                    let container = vec![
                            serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("# {} - {}", $action_name, user.backup_name))),
                            $($element),*
                        ];

                    $(let container = {
                        let mut container = container;
                        container.extend($additional);
                        container
                    };)*

                    let body:serenity::builder::CreateMessage =
                        serenity::builder::CreateMessage::new()
                            .flags(serenity::model::channel::MessageFlags::IS_COMPONENTS_V2)
                            .components(vec![
                                serenity::builder::CreateComponent::Container(serenity::builder::CreateContainer::new(container))
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
        let invite=|locs: &[&str], olocs: &[Option<&str>]|{
            let mut data = vec![];
            for loc in locs.into_iter().chain(olocs.into_iter().filter_map(Option::as_ref)) {
                if let Some((world, instance)) = loc.split_once(":") {
                    let id = Id::InviteMe {i: self.i, vrc_user_id: self.config.vrc_user_id.clone(), world: world.to_string(), instance: instance.to_string()};
                    match serde_json::to_string(&id) {
                        Ok(v) => {
                            data.push(serenity::builder::CreateButton::new(v).label("Invite Myself"));
                        },
                        Err(err) => {
                            log::error!("Failed to serialize Id {id:?}: {err}");
                        }
                    }
                }
            }

            let size = data.len();
            (serenity::builder::CreateContainerComponent::ActionRow(serenity::builder::CreateActionRow::Buttons(data.into())), size)
        };
        let user = |usr:&vrchatapi::models::User| {
            let mut data = vec![];

            if let Some(location) = &usr.location { data.push(serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Location: {}", location)))); }
            if let Some(world_id) = &usr.world_id { data.push(serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("World Id: {}", world_id)))); }
            if let Some(instance_id) = &usr.instance_id { data.push(serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Instance Id: {}", instance_id)))); }
            if let Some(traveling_to_world) = &usr.traveling_to_world { data.push(serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Traveling to World: {}", traveling_to_world)))); }
            if let Some(traveling_to_location) = &usr.traveling_to_location { data.push(serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Traveling to Location: {}", traveling_to_location)))); }
            if let Some(traveling_to_instance) = &usr.traveling_to_instance { data.push(serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Traveling to Instance: {}", traveling_to_instance)))); }
            let (row, len) = invite(&[], &[
                usr.location.as_ref().map(String::as_str),
                usr.traveling_to_location.as_ref().map(String::as_str),
            ]);
            if len > 0 {
                data.push(row);
            }

            data
        };

        let (raw_message, ws_message) = match message_arg {
            Ok((raw, msg)) => (raw, Some(msg)),
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
                log::error!("failed receiving ws element: {error}");
                (message, None)
            },
            Err(WSElementError::UnsupportedMessageType(data)) => {
                log::error!("failed receiving ws element: UnsupportedMessageType: {data:?}");
                return
            },
            Err(WSElementError::WebSocketError(err)) => {
                log::error!("failed receiving ws element: {err}");
                return
            }
        };
        {
            let ws_file = self.ws_log_file.clone();
            let data = {
                let mut data = data_encoding::BASE64.encode(raw_message.as_payload().as_ref());
                data.push('\n');
                data
            };
            tokio::task::spawn(async move {
                let mut file = ws_file.lock().await;
                if let Err(err) = file.write_all(data.as_bytes()).await {
                    log::error!("Failed to write ws data to log. Error: {err}, data: {data}");
                    let _ = file.write_all(&[0]).await;
                }
                if let Err(err) = file.flush().await {
                    log::error!("Failed to flush ws data to log. Error: {err}, data: {data}");
                }
            });
        }
        let ws_message = match ws_message {
            Some(v) => v,
            None => return,
        };
        match ws_message {
            WebsocketMessage::FriendAdd(vrchatapi::models::WebsocketFriendAddEncoded{ content, r#type: _ }) => handle_msg!(&*content.user_id, content.user, "FriendAdd",),
            WebsocketMessage::FriendDelete(vrchatapi::models::WebsocketFriendDeleteEncoded{ content, r#type: _ }) => handle_msg!(&*content.user_id, "FriendDelete",),
            WebsocketMessage::FriendOnline(vrchatapi::models::WebsocketFriendOnlineEncoded{ content, r#type: _ }) => handle_msg!(&*content.user_id, "FriendOnline",
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Platform: {}", content.platform))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Location: {}", content.location))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Can Request Invite: {}", content.can_request_invite)));
                [invite(&[&content.location], &[]).0]
            ),
            WebsocketMessage::FriendActive(vrchatapi::models::WebsocketFriendActiveEncoded{ content, r#type: _ }) => handle_msg!(&*content.user_id, content.user, "FriendActive",
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Platform: {}", content.platform)))
            ),
            WebsocketMessage::FriendOffline(vrchatapi::models::WebsocketFriendOfflineEncoded{ content, r#type: _ }) => handle_msg!(&*content.user_id, "FriendOffline",
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Platform: {}", content.platform)))
            ),
            WebsocketMessage::FriendUpdate(vrchatapi::models::WebsocketFriendUpdateEncoded{ content, r#type: _}) => handle_msg!(&*content.user_id, content.user, "FriendUpdate",;
                user(&content.user)
            ),
            WebsocketMessage::FriendLocation(vrchatapi::models::WebsocketFriendLocationEncoded{ content, r#type: _ }) => handle_msg!(&*content.user_id, content.user, "FriendLocation",
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Location: {}", content.location))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Traveling to Location: {}", content.traveling_to_location))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("World Id: {}", content.world_id))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Can Request Invite: {}", content.can_request_invite)));
                [invite(&[content.location.as_str(), content.traveling_to_location.as_str()], &[]).0]
            ),
            WebsocketMessage::UserUpdate { .. } => {}
            WebsocketMessage::UserLocation { .. } => {}
            WebsocketMessage::UserBadgeAssigned { .. } => {}
            WebsocketMessage::UserBadgeUnassigned { .. } => {}
            WebsocketMessage::Notification { .. } => {}
            WebsocketMessage::ResponseNotification { .. } => {}
            WebsocketMessage::SeeNotification { .. } => {}
            WebsocketMessage::HideNotification { .. } => {}
            WebsocketMessage::ClearNotification { .. } => {}
            WebsocketMessage::NotificationV2 { .. } => {}
            WebsocketMessage::NotificationV2Update { .. } => {}
            WebsocketMessage::NotificationV2Delete { .. } => {}
            WebsocketMessage::ContentRefresh { .. } => {}
            WebsocketMessage::InstanceQueueJoined { .. } => {}
            WebsocketMessage::InstanceQueueReady { .. } => {}
            WebsocketMessage::GroupJoined { .. } => {}
            WebsocketMessage::GroupLeft { .. } => {}
            WebsocketMessage::GroupMemberUpdated { .. } => {}
            WebsocketMessage::GroupRoleUpdated { .. } => {}
            WebsocketMessage::WebsocketMessageUnknown(data) => {
                let dc = self.dc.clone();
                let id = self.owner;
                let data = vec![
                    serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new("# Failed to Deserialize Websocket Message")),
                    serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Data in Debug: `{data:?}`"))),
                    match serde_json::to_string(&data) {
                        Ok(v) => serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Data Deserialized: ```\n{v}\n```", ))),
                        Err(err) => serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Data Deserialized: Error: `{err}`", ))),
                    },
                    serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Raw Data: ```\n{}\n```", String::from_utf8_lossy(raw_message.as_payload().as_ref()))))
                ];
                let body = serenity::builder::CreateMessage::new()
                    .flags(serenity::model::channel::MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![
                        serenity::builder::CreateComponent::Container(serenity::builder::CreateContainer::new(data))
                    ]);
                tokio::task::spawn(async move {
                    if let Err(err) = id.direct_message(dc, body).await {
                        log::error!("Failed to send message to {id}: {err}")
                    }
                });
            }
        }
    }
}

#[derive(Debug, Clone, serde_derive::Deserialize, serde_derive::Serialize)]
enum Id{
    InviteMe{i: usize, vrc_user_id: Arc<str>, world: String, instance: String}
}

struct EventHandler{
    configs: Box<[(Config, Arc<vrc::Vrc>)]>
}
#[serenity::async_trait]
impl serenity::prelude::EventHandler for EventHandler {
    async fn dispatch(&self, context: &Context, event: &FullEvent) {
        match event {
            FullEvent::InteractionCreate {
                interaction: Interaction::Component(component),
                ..
            } => {

                let respond = async |v:Cow<'static, str>|{
                    match component.create_response(
                        &*context.http,
                        serenity::builder::CreateInteractionResponse::Message(serenity::builder::CreateInteractionResponseMessage::new().content(v.clone()).ephemeral(true))
                    ).await {
                        Ok(()) => {},
                        Err(err) => {
                            log::error!("Failed to send error message for interaction: {err}\n\tOriginal Error: {v}");
                        }
                    }
                };

                let id = match serde_json::from_str::<Id>(&component.data.custom_id) {
                    Ok(v) => v,
                    Err(err) => {
                        respond(format!("Error deserializing id of Button: {err}").into()).await;
                        return;
                    }
                };

                match id {
                    Id::InviteMe { i, vrc_user_id, world, instance } => {
                        let pred = |v: &&(Config, Arc<vrc::Vrc>)|v.0.vrc_user_id == vrc_user_id;
                        let usr = self.configs.get(i)
                            .filter(pred)
                            .map_or_else(||{
                                self.configs.iter().filter(pred).next()
                            }, Some);
                        let (cfg, vrc) = match usr {
                            Some(v) => v,
                            None => {
                                respond("Did not find vrchat account to invite".into()).await;
                                return;
                            }
                        };

                        if cfg.account_owners.get(&component.user.id).is_none() {
                            respond("You are NOT listed as one of the VRChat account owners. You don't have permission to send an invite to the VRChat account.".into()).await;
                            return;
                        }

                        match vrchatapi::apis::invite_api::invite_myself_to(&vrc.get_configuration().await, &world, &instance).await {
                            Ok(_) => {
                                respond(
                                    "Sent invite".into()
                                ).await;
                            },
                            Err(err) => {
                                respond(
                                    format!("Failed to send invite: {err}").into()
                                ).await;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn async_main() -> anyhow::Result<()> {
    let ws_log = Arc::new(Mutex::new(tokio::fs::File::options().append(true).create(true).open("wslog.bin").await?));
    let config: ConfigFile = serde_json::from_slice(&tokio::fs::read("config.json").await?)?;
    let ConfigFile{configs, owner} = config;
    let vrc = configs.into_iter().map(|VRCConfig{vrchat_cookies, config}|{
        (config, Arc::new(vrc::Vrc::new(vrchat_cookies.into_iter()).expect("Failed to create VRChat instance")))
    }).collect::<Vec<_>>();

    let mut settings = serenity::cache::Settings::default();
    settings.cache_guilds = false;
    settings.max_messages = 0;
    settings.cache_users = true;
    let mut client = serenity::Client::builder(serenity::all::Token::from_env("DISCORD_TOKEN")?, serenity::prelude::GatewayIntents::empty())
        .cache_settings(settings)
        .event_handler(Arc::new(EventHandler{
            configs: vrc.clone().into_boxed_slice()
        }))
        .await?
    ;

    {
        let shutdown = client.shard_manager.get_shutdown_trigger();
        tokio::task::spawn(async{
            tokio::signal::ctrl_c().await.unwrap();
            shutdown();
        });
    }

    for (i, (config, vrc)) in vrc.into_iter().enumerate() {
        let handler = Arc::new(Mutex::new(Handler {
            ws_log_file: ws_log.clone(),
            i,
            config,
            owner,
            dc: CacheHttp {
                http: client.http.clone(),
                cache: client.cache.clone(),
            },
            vrc: vrc.clone(),
        }));
        match vrchatapi::apis::playermoderation_api::get_player_moderations(&vrc.get_configuration().await, None, None).await {
            Ok(v) => {log::info!("Got player Moderations: {v:?}"); }
            Err(err) => { log::warn!("Failed to get player Moderations. Are we actually logged in? {err}"); }
        }

        match vrc.check_auth(handler.lock_owned().await).await.expect("VRChat isn't logged in") {
            CurrentUserLoginResponse::CurrentUser(_) => {}
            CurrentUserLoginResponse::RequiresTwoFactorAuth(_) => {
                eprintln!("VRChat wants 2fa");
                return Err(Error::LoginError(LoginError::Needs2fa).into());
            }
        }
    }


    client.start_autosharded().await?;
    Ok(())
}