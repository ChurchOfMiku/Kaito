use anyhow::Result;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

use songbird::{
    input::{Input, Restartable},
    tracks::{PlayMode, TrackHandle, TrackState},
    Bitrate, Call,
};

use crate::services::{ChannelId, ServerId, VoiceConnection};

use super::DiscordService;

pub struct DiscordVoiceConnection {
    call: Arc<Mutex<Call>>,
    track_handle: Mutex<Option<TrackHandle>>,
    server_id: u64,
    channel_id: u64,
}

#[async_trait]
impl VoiceConnection<DiscordService> for DiscordVoiceConnection {
    fn channel_id(&self) -> ChannelId {
        ChannelId::Discord(self.channel_id)
    }

    fn server_id(&self) -> ServerId {
        ServerId::Discord(self.server_id)
    }

    async fn position(&self) -> Option<Duration> {
        if let Some(track_handle) = self.track_handle.lock().await.as_ref() {
            if let Ok(info) = track_handle.get_info().await {
                Some(info.position)
            } else {
                None
            }
        } else {
            None
        }
    }

    async fn length(&self) -> Option<Duration> {
        None
    }

    async fn playing(&self) -> bool {
        if let Some(track_handle) = self.track_handle.lock().await.as_ref() {
            if let Ok(info) = track_handle.get_info().await {
                info.playing == PlayMode::Play
            } else {
                false
            }
        } else {
            false
        }
    }

    async fn connected(&self) -> bool {
        self.call.lock().await.current_connection().is_some()
    }

    async fn set_volume(&self, volume: f32) {
        if let Some(track_handle) = self.track_handle.lock().await.as_ref() {
            track_handle.set_volume(volume).ok();
        }
    }

    async fn play(&self, url: &str, seek: Option<Duration>) -> Result<()> {
        let mut input: Input = Restartable::ytdl(url.to_string(), false)
            .await
            .map_err(|err| anyhow::anyhow!("{:?}", err))?
            .into();

        if let Some(seek) = seek {
            input.seek_time(seek);
        }

        let mut call = self.call.lock().await;
        call.set_bitrate(Bitrate::Max);

        let track_handle = call.play_only_source(input);
        *self.track_handle.lock().await = Some(track_handle);

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.call.lock().await.stop();
        *self.track_handle.lock().await = None;

        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.call.lock().await.leave().await?;

        Ok(())
    }
}

impl DiscordVoiceConnection {
    pub fn new(server_id: u64, channel_id: u64, call: Arc<Mutex<Call>>) -> DiscordVoiceConnection {
        DiscordVoiceConnection {
            server_id,
            channel_id,
            call,
            track_handle: Mutex::new(None),
        }
    }
}
