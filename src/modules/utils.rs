use anyhow::Result;
use async_mutex::Mutex;
use lru::LruCache;
use rand::{distributions, Rng};
use std::sync::Arc;
use tokio::fs;

use super::{Module, ModuleKind};
use crate::{
    bot::Bot,
    message::MessageSettings,
    services::{Channel, ChannelId, Message, MessageId, Server, ServerId, Service, User},
    settings::prelude::*,
};

pub struct UtilsModule {
    bot: Arc<Bot>,
    last_generated_messages: Mutex<LruCache<MessageId, (ChannelId, MessageId)>>,
    settings: Arc<UtilsModuleSettings>,
}

settings! {
    UtilsModuleSettings,
    UtilsModule,
    {
        extract_media_urls: bool => (false, SettingFlags::empty(), "Extract media urls out of tweets", [])
    }
}

pub static IMAGE_EXTENSIONS: &[&'static str] = &[".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"];

#[async_trait]
impl Module for UtilsModule {
    const KIND: ModuleKind = ModuleKind::Utils;
    const ID: &'static str = "utils";
    const NAME: &'static str = "Utils";

    type ModuleConfig = ();
    type ModuleSettings = UtilsModuleSettings;

    async fn load(bot: Arc<Bot>, _config: ()) -> Result<Arc<UtilsModule>> {
        Ok(Arc::new(UtilsModule {
            bot: bot.clone(),
            last_generated_messages: Mutex::new(LruCache::new(64)),
            settings: UtilsModuleSettings::create(bot)?,
        }))
    }

    async fn unload(&self) -> Result<()> {
        Ok(())
    }

    async fn message(&self, msg: Arc<dyn Message<impl Service>>) -> Result<()> {
        lazy_static::lazy_static! {
            static ref REDDIT_RE: regex::Regex = regex::Regex::new(r#"https?://(?:(?:old.|www.)?reddit.com|v.redd.it)/.+(?: )?"#).unwrap();
            static ref TIKTOK_RE: regex::Regex = regex::Regex::new(r#"https?://(?:www.|vm.)?tiktok.com/.+(?: )?"#).unwrap();
            static ref TWITTER_RE: regex::Regex = regex::Regex::new(r#"https?://(?:www.)?twitter.com/.+/status(?:es)?/(\d+)(?:.+ )?"#).unwrap();

            /// Convert media.discordapp.net to cdn.discordapp.com
            static ref DISCORD_MEDIA_VIDEO_RE: regex::Regex = regex::Regex::new(r#"https?:\/\/media.discordapp.net\/attachments\/\d+\/\d+\/\S+\.(?:mp4|mov|webm|mkv|flv|wmv|avi|mxf|mpg)"#).unwrap();
        }

        #[derive(Clone, Copy)]
        enum MediaService {
            Reddit,
            Tiktok,
            Twitter,
            DiscordMediaVideoLink
        }

        impl MediaService {
            pub fn should_download(self) -> bool {
                match self {
                    MediaService::Reddit => true,
                    MediaService::Tiktok => true,
                    _ => false,
                }
            }
        }

        let mut matches: Vec<_> = REDDIT_RE
            .captures_iter(msg.content())
            .map(|cap| (MediaService::Reddit, cap))
            .collect();
        matches.append(
            &mut TIKTOK_RE
                .captures_iter(msg.content())
                .map(|cap| (MediaService::Tiktok, cap))
                .collect(),
        );
        matches.append(
            &mut TWITTER_RE
                .captures_iter(msg.content())
                .map(|cap| (MediaService::Twitter, cap))
                .collect(),
        );
        matches.append(
            &mut DISCORD_MEDIA_VIDEO_RE
                .captures_iter(msg.content())
                .map(|cap| (MediaService::DiscordMediaVideoLink, cap))
                .collect(),
        );

        if !matches.is_empty() {
            let channel = msg.channel().await?;
            let server = channel.server().await?;
            let extract_media_urls = self
                .settings
                .extract_media_urls
                .value(server.id(), channel.id())
                .await?;

            if extract_media_urls {
                let mut out = Vec::new();
                let mut attachment = None;

                for (service, status_match) in matches {
                    let media_url = status_match.get(0).unwrap().as_str();
                    let should_download = service.should_download();

                    // Convert media.discordapp.net to cdn.discordapp.com
                    if service == MediaService::DiscordMediaVideoLink {
                        out.push(media_url.replace("media.discordapp.net", "cdn.discordapp.com"));
                        continue;
                    }

                    let output = tokio::process::Command::new("youtube-dl")
                        .arg("-g")
                        .arg("-f")
                        .arg("best")
                        .arg("--no-warnings")
                        .arg(media_url)
                        .output()
                        .await;

                    match output {
                        Ok(output) => {
                            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();

                            // Drop converting images
                            let is_image = IMAGE_EXTENSIONS.iter().any(|ext| url.ends_with(ext));
                            if is_image {
                                continue;
                            }

                            // Let the next section download the media instead since we cannot get a viewable direct link to the media
                            if !should_download {
                                if !url.is_empty() {
                                    out.push(url);
                                }
                            }
                        }
                        Err(err) => {
                            println!("Error from youtube-dl url extraction: {}", err.to_string())
                        }
                    }

                    if should_download {
                        let name: String = rand::thread_rng()
                            .sample_iter(&distributions::Alphanumeric)
                            .take(6)
                            .map(char::from)
                            .collect();

                        let out_path = std::env::temp_dir().join(format!("{}.mp4", name));

                        // TODO: Move to pipeing to stdout when youtube-dl can support do it after muxing
                        tokio::process::Command::new("youtube-dl")
                                .args(&[
                                    "-f", "bestvideo[filesize<6MB]+bestaudio[filesize<2MB]/best/bestvideo+bestaudio",
                                    "--merge-output-format", "mp4",
                                    "--ignore-config",
                                    "--no-playlist",
                                    "--no-warnings",
                                    media_url,
                                    "-o", &out_path.to_string_lossy()
                                    ])
                                .output().await?;

                        if out_path.exists() {
                            attachment = Some(fs::read(&out_path).await?);
                            fs::remove_file(&out_path).await?;
                        }
                    }
                }

                if !out.is_empty() || attachment.is_some() {
                    let reply_msg = channel
                        .send(
                            out.join("\n"),
                            if let Some(attachment) = attachment {
                                MessageSettings {
                                    attachments: vec![("video.mp4".into(), attachment)],
                                    ..MessageSettings::default()
                                }
                            } else {
                                MessageSettings::default()
                            },
                        )
                        .await?;

                    self.last_generated_messages
                        .lock()
                        .await
                        .put(msg.id(), (channel.id(), reply_msg.id()));
                }
            }
        }

        Ok(())
    }

    async fn message_update(
        &self,
        _msg: Arc<dyn Message<impl Service>>,
        _old_msg: Option<Arc<dyn Message<impl Service>>>,
    ) -> Result<()> {
        Ok(())
    }

    async fn message_delete(
        &self,
        _server_id: Option<ServerId>,
        _channel_id: ChannelId,
        message_id: MessageId,
    ) -> Result<()> {
        let mut last_messages = self.last_generated_messages.lock().await;

        if let Some((channel_id, reply_id)) = last_messages.pop(&message_id) {
            self.bot
                .get_ctx()
                .services()
                .delete_message(channel_id, reply_id)
                .await?;
        }

        Ok(())
    }

    async fn reaction(
        &self,
        _msg: Arc<dyn Message<impl Service>>,
        _reactor: Arc<dyn User<impl Service>>,
        _reaction: String,
        _remove: bool,
    ) -> Result<()> {
        Ok(())
    }

    async fn enabled(&self, _server_id: ServerId, _channel_id: ChannelId) -> Result<bool> {
        Ok(true)
    }

    fn settings(&self) -> &Arc<UtilsModuleSettings> {
        &self.settings
    }
}
