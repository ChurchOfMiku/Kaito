use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

macro_rules! settings {
    ($sname:ident, { $($name:ident: $type:ty => ($default:expr, $flags:expr, $help:expr, [ $($setting_ident:ident => $setting_value:expr)* ])),* }) => {
        pub struct $sname {
            $(
                pub $name: crate::settings::Setting<$type>,
            )*
        }

        impl $sname {
            pub fn create() -> Result<Arc<$sname>> {
                $(
                    #[allow(unused, non_camel_case_types)]
                    type $name = <bool as SettingValue>::Parameters;
                )*

                Ok(Arc::new($sname {
                    $(
                        $name: Setting::create(stringify!($name).into(), $default, $name {}, $flags, $help.into())?,
                    )*
                }))
            }
        }
    };
    (_parameters, bool, { $($key:ident: $value:expr),+ }) => {
        SettingBoolParameters {
            $($key: $value),+
        }
    }
}

bitflags! {
    pub struct SettingFlags: u8 {
        const _STUB = 1;
    }
}

pub struct Setting<T>
where
    T: SettingValue + Send + Sync,
    T::Parameters: Send + Sync,
{
    name: String,
    flags: SettingFlags,
    help: String,
    parameters: T::Parameters,
    value: RwLock<T>,
}

impl<T> Setting<T>
where
    T: SettingValue + Send + Sync,
    T::Parameters: Send + Sync,
{
    pub fn create(
        name: &str,
        value: T,
        parameters: T::Parameters,
        flags: SettingFlags,
        help: String,
    ) -> Result<Setting<T>> {
        SettingValue::is_valid(&value, &parameters)?;

        Ok(Setting {
            name: name.to_string(),
            value: RwLock::new(value),
            parameters: parameters,
            flags,
            help,
        })
    }
}

pub trait SettingValue: Clone + Sized + Deserialize<'static> + Serialize {
    type Parameters;

    // Let the value type check that the default value is valid based on the paramters
    fn is_valid(value: &Self, parameters: &Self::Parameters) -> Result<()>;
    // Set
    fn set_value(input: &str, parameters: &Self::Parameters) -> Result<Self>;
}

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

#[derive(Debug, Copy, Clone)]
pub enum SettingType {
    Bool,
}

#[derive(Debug, Error)]
pub enum SettingError {
    #[error("Unable to parse \"{}\" as {:?}", input, expected)]
    UnexpectedInput {
        expected: SettingType,
        input: String,
    },
}

pub mod prelude {
    pub use super::{Setting, SettingBoolParameters, SettingFlags, SettingValue};
}
