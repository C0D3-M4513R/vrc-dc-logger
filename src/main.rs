#![forbid(unsafe_code)]
mod vrc;
mod error;

use std::borrow::Cow;
use std::sync::Arc;
use serenity::all::{Context, FullEvent, Interaction};
use tokio::sync::Mutex;
pub(crate) use error::{VRCError, Error, impl_from};

pub use tokio::task::spawn as spawn;
pub use tokio::task::spawn_local as spawn_local;
pub use tokio::task::spawn_blocking as spawn_blocking;
use vrchatapi::models::RegisterUserAccount200Response;
use crate::vrc::LoginError;
use crate::vrc::websocket::connection::WSElementError;
use crate::vrc::websocket::location::Location;
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
            ($uid:expr, $user:expr, $action_name:literal, $($element:expr),* $(; $additional:expr),*) => {
                if let Some(user) = self.config.tracking_users.get($uid) {
                    let vrc_user = &$user;
                    let container = vec![
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
        let invite=|locs: &[Location], olocs: &[Option<Location>]|{
            let mut data = vec![];
            for loc in locs.into_iter().chain(olocs.into_iter().filter_map(Option::as_ref)) {
                if let Location::Other(v) = loc {
                    if let Some((world, instance)) = v.split_once(":") {
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
                usr.location.as_ref().map(|v|Location::from(&**v)),
                usr.traveling_to_location.as_ref().map(|v|Location::from(&**v)),
            ]);
            if len > 0 {
                data.push(row);
            }

            data
        };
        match message {
            Ok(Message::FriendAdd { content }) => handle_msg!(&*content.user_id, content.user, "FriendAdd",),
            Ok(Message::FriendDelete { content }) => handle_msg!(&*content.user_id, "FriendDelete",),
            Ok(Message::FriendOnline { content }) => handle_msg!(&*content.user_id, content.user, "FriendOnline",
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Platform: {}", content.platform))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Location: {}", content.location))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Can Request Invite: {}", content.can_request_invite)));
                [invite(&[content.location], &[]).0]
            ),
            Ok(Message::FriendActive { content }) => handle_msg!(&*content.user_id, content.user, "FriendActive",
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Platform: {}", content.platform)))
            ),
            Ok(Message::FriendOffline { content }) => handle_msg!(&*content.user_id, "FriendOffline",
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Platform: {}", content.platform)))
            ),
            Ok(Message::FriendUpdate { content }) => handle_msg!(&*content.user_id, content.user, "FriendUpdate",;
                user(&content.user)
            ),
            Ok(Message::FriendLocation { content }) => handle_msg!(&*content.user_id, content.user, "FriendLocation",
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Location: {}", content.location))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Traveling to Location: {}", content.traveling_to_location))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("World Id: {}", content.world_id))),
                serenity::builder::CreateContainerComponent::TextDisplay(serenity::builder::CreateTextDisplay::new(format!("Can Request Invite: {}", content.can_request_invite)));
                [invite(&[content.location, content.traveling_to_location], &[]).0]
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
            i,
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