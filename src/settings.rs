use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{marker::PhantomData, str::FromStr, sync::Arc};
use thiserror::Error;

use crate::{
    bot::Bot,
    modules::Module,
    services::{ChannelId, ServerId},
};

macro_rules! settings {
    ($sname:ident, $module:ident, { $($name:ident: $type:ty => ($default:expr, $flags:expr, $help:expr, [ $($setting_ident:ident => $setting_value:expr)* ])),* }) => {
        pub struct $sname {
            $(
                pub $name: crate::settings::Setting<$type, $module>,
            )*
        }

        impl $sname {
            pub fn create(bot: Arc<Bot>) -> Result<Arc<$sname>> {
                $(
                    #[allow(unused, non_camel_case_types)]
                    type $name = <$type as SettingValue>::Parameters;
                )*

                Ok(Arc::new($sname {
                    $(
                        $name: Setting::create(bot.clone(), stringify!($name).into(), $default, $name {
                            $($setting_ident: Some($setting_value),)*
                            ..Default::default()
                        }, $flags, $help.into())?,
                    )*
                }))
            }
        }
    };
}

bitflags! {
    pub struct SettingFlags: u8 {
        const SERVER_OVERRIDE = 1;
    }
}

pub struct Setting<T, M>
where
    T: SettingValue + Send + Sync,
    T::Parameters: Send + Sync,
    M: Module + Send + Sync,
{
    bot: Arc<Bot>,
    name: String,
    flags: SettingFlags,
    help: String,
    parameters: T::Parameters,
    default: T,
    _phantom: PhantomData<M>,
}

impl<T, M> Setting<T, M>
where
    T: SettingValue + Send + Sync,
    T::Parameters: Send + Sync,
    M: Module + Send + Sync,
{
    pub fn create(
        bot: Arc<Bot>,
        name: &str,
        default: T,
        parameters: T::Parameters,
        flags: SettingFlags,
        help: String,
    ) -> Result<Setting<T, M>> {
        SettingValue::is_valid(&default, &parameters)?;

        Ok(Setting {
            bot,
            name: name.to_string(),
            parameters: parameters,
            flags,
            help,
            default,
            _phantom: PhantomData::default(),
        })
    }

    pub async fn value(&self, server_id: ServerId, channel_id: ChannelId) -> Result<T> {
        if self.flags.contains(SettingFlags::SERVER_OVERRIDE) {
            if let Some(value) = self.get_server_value(server_id).await? {
                return Ok(value);
            }

            if let Some(value) = self.get_channel_value(channel_id).await? {
                return Ok(value);
            }

            Ok(self.default.clone())
        } else {
            if let Some(value) = self.get_channel_value(channel_id).await? {
                return Ok(value);
            }

            if let Some(value) = self.get_server_value(server_id).await? {
                return Ok(value);
            }

            Ok(self.default.clone())
        }
    }

    async fn get_channel_value(&self, channel_id: ChannelId) -> Result<Option<T>> {
        let raw_value = match self
            .bot
            .db()
            .get_channel_setting(channel_id, &format!("{}/{}", M::ID, self.name))
            .await?
        {
            Some(v) => v,
            None => return Ok(None),
        };

        // Just go back to default if the raw value is invalid
        Ok(T::set_value(&raw_value, &self.parameters).ok())
    }

    async fn get_server_value(&self, server_id: ServerId) -> Result<Option<T>> {
        let raw_value = match self
            .bot
            .db()
            .get_server_setting(server_id, &format!("{}/{}", M::ID, self.name))
            .await?
        {
            Some(v) => v,
            None => return Ok(None),
        };

        // Just go back to default if the raw value is invalid
        Ok(T::set_value(&raw_value, &self.parameters).ok())
    }

    pub async fn set_value(&self, ctx: SettingContext, input: &str) -> Result<()> {
        // Ensure the value is valid
        let _value = T::set_value(input, &self.parameters)?;

        match ctx {
            SettingContext::Channel(channel_id) => {
                self.bot
                    .db()
                    .save_channel_setting(channel_id, &format!("{}/{}", M::ID, self.name), input)
                    .await?;
            }
            SettingContext::Server(server_id) => {
                self.bot
                    .db()
                    .save_server_setting(server_id, &format!("{}/{}", M::ID, self.name), input)
                    .await?;
            }
        };

        Ok(())
    }
}

pub trait SettingValue: Clone + Sized + Deserialize<'static> + Serialize {
    type Parameters;

    // Let the value type check that the default value is valid based on the paramters
    fn is_valid(value: &Self, parameters: &Self::Parameters) -> Result<()>;
    // Set
    fn set_value(input: &str, parameters: &Self::Parameters) -> Result<Self>;
}

// Setting value - bool

impl SettingValue for bool {
    type Parameters = SettingBoolParameters;

    fn is_valid(_value: &bool, _parameters: &SettingBoolParameters) -> Result<()> {
        Ok(())
    }

    fn set_value(input: &str, _parameters: &SettingBoolParameters) -> Result<bool> {
        let trimmed = input.trim();

        if trimmed == "true" {
            return Ok(true);
        } else if trimmed == "false" {
            return Ok(false);
        }

        if let Some(val) = u32::from_str(trimmed).ok() {
            return Ok(val > 0);
        }

        Err(SettingError::UnexpectedInput {
            expected: SettingType::Bool,
            input: input.into(),
        }
        .into())
    }
}

#[derive(Default)]
pub struct SettingBoolParameters {}

// Setting value - String

impl SettingValue for String {
    type Parameters = SettingStringParameters;

    fn is_valid(value: &String, parameters: &SettingStringParameters) -> Result<()> {
        if let Some(max_len) = parameters.max_len {
            let len = value.len();
            if len > max_len {
                return Err(SettingError::ExceededMaxLength {
                    max: max_len,
                    length: len,
                }
                .into());
            }
        }

        Ok(())
    }

    fn set_value(input: &str, parameters: &SettingStringParameters) -> Result<String> {
        <String as SettingValue>::is_valid(&input.into(), parameters)?;

        Ok(input.into())
    }
}

#[derive(Default)]
pub struct SettingStringParameters {
    pub max_len: Option<usize>,
}

pub enum SettingContext {
    Channel(ChannelId),
    Server(ServerId),
}

#[derive(Debug, Copy, Clone)]
pub enum SettingType {
    Bool,
}

#[derive(Debug, Error)]
pub enum SettingError {
    #[error("unable to parse \"{}\" as {:?}", input, expected)]
    UnexpectedInput {
        expected: SettingType,
        input: String,
    },
    #[error("len {} exceeded max length {}", length, max)]
    ExceededMaxLength { max: usize, length: usize },
}

pub mod prelude {
    pub use super::{Setting, SettingBoolParameters, SettingFlags, SettingValue};
}
