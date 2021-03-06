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
            static ref REDDIT_RE: regex::Regex = regex::Regex::new(r#"https?://(?:old.|www.)?reddit.com/.+(?: )?"#).unwrap();
            static ref TIKTOK_RE: regex::Regex = regex::Regex::new(r#"https?://(?:www.|vm.)?tiktok.com/.+(?: )?"#).unwrap();
            static ref TWITTER_RE: regex::Regex = regex::Regex::new(r#"https?://(?:www.)?twitter.com/.+/status(?:es)?/(\d+)(?:.+ )?"#).unwrap();
        }

        #[derive(Clone, Copy)]
        enum MediaService {
            Reddit,
            Tiktok,
            Twitter,
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
                    if service.should_download() {
                        if attachment.is_some() {
                            continue;
                        }

                        let name: String = rand::thread_rng()
                            .sample_iter(&distributions::Alphanumeric)
                            .take(6)
                            .map(char::from)
                            .collect();

                        let out_path = std::env::temp_dir().join(format!("{}.mp4", name));

                        // TODO: Move to pipeing to stdout when youtube-dl can support do it after muxing
                        tokio::process::Command::new("youtube-dl")
                                .args(&[
                                    "-f", "bestvideo[filesize<8MB]+bestaudio[filesize<2MB]/best/bestvideo+bestaudio",
                                    "--merge-output-format", "mp4",
                                    "--ignore-config",
                                    "--no-playlist",
                                    "--no-warnings",
                                    status_match.get(0).unwrap().as_str(),
                                    "-o", &out_path.to_string_lossy()
                                    ])
                                .output().await?;

                        if out_path.exists() {
                            attachment = Some(fs::read(&out_path).await?);
                            fs::remove_file(&out_path).await?;
                        }
                    } else {
                        let output = tokio::process::Command::new("youtube-dl")
                            .arg("-g")
                            .arg("-f")
                            .arg("best")
                            .arg("--no-warnings")
                            .arg(status_match.get(0).unwrap().as_str())
                            .output()
                            .await;

                        match output {
                            Ok(output) => {
                                //println!("{}", String::from_utf8_lossy(&output.stderr).to_string());
                                let url = String::from_utf8_lossy(&output.stdout).to_string();
                                if !url.is_empty() {
                                    out.push(url);
                                }
                            }
                            Err(err) => println!(
                                "Error from youtube-dl twitter extraction: {}",
                                err.to_string()
                            ),
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
