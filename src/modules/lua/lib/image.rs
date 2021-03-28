use anyhow::Result;
use crossbeam::channel::Sender;
use graphicsmagick::{
    types,
    wand::{MagickWand, PixelWand},
};
use mlua::{
    prelude::{FromLua, LuaError, LuaMultiValue, LuaString, LuaTable, LuaValue},
    Lua, MetaMethod, UserData, UserDataMethods,
};
use std::sync::Arc;
use tokio::task::JoinError;

use crate::modules::lua::state::LuaAsyncCallback;

macro_rules! magick_enum {
    ($name:ident, $inner:ty, $num_ty:ty, { $($enum_ident:ident,)+ }) => {
        pub struct $name($inner);

        impl $name {
            pub fn create_table(state: &Lua) -> Result<LuaTable> {
                let tbl = state.create_table()?;

                $(
                    tbl.set(stringify!($enum_ident), <$inner>::$enum_ident as $num_ty)?;
                )+

                Ok(tbl)
            }

            pub fn inner(&self) -> $inner {
                self.0
            }
        }

        $(
            #[allow(non_upper_case_globals)]
            const $enum_ident: $num_ty = <$inner>::$enum_ident as $num_ty;
        )+

        impl<'lua> FromLua<'lua> for $name  {
            fn from_lua(value: LuaValue<'lua>, lua: &'lua Lua) -> Result<$name, LuaError> {
                let number: $num_ty = FromLua::from_lua(value, lua)?;

                #[allow(non_upper_case_globals)]
                match number {
                    $(
                        $enum_ident => Ok($name(<$inner>::$enum_ident)),
                    )+
                    _ => Err(LuaError::FromLuaConversionError {
                        from: stringify!($num_ty),
                        to: stringify!($name),
                        message: None,
                    }),
                }
            }
        }
    };
}

magick_enum! {
    ChannelType,
    types::ChannelType,
    u32,
    {
        UndefinedChannel,
        RedChannel,
        CyanChannel,
        GreenChannel,
        MagentaChannel,
        BlueChannel,
        YellowChannel,
        OpacityChannel,
        BlackChannel,
        MatteChannel,
        AllChannels,
        GrayChannel,
    }
}

magick_enum! {
    CompositeOperator,
    types::CompositeOperator,
    u32,
    {
        UndefinedCompositeOp,
        OverCompositeOp,
        InCompositeOp,
        OutCompositeOp,
        AtopCompositeOp,
        XorCompositeOp,
        PlusCompositeOp,
        MinusCompositeOp,
        AddCompositeOp,
        SubtractCompositeOp,
        DifferenceCompositeOp,
        MultiplyCompositeOp,
        BumpmapCompositeOp,
        CopyCompositeOp,
        CopyRedCompositeOp,
        CopyGreenCompositeOp,
        CopyBlueCompositeOp,
        CopyOpacityCompositeOp,
        ClearCompositeOp,
        DissolveCompositeOp,
        DisplaceCompositeOp,
        ModulateCompositeOp,
        ThresholdCompositeOp,
        NoCompositeOp,
        DarkenCompositeOp,
        LightenCompositeOp,
        HueCompositeOp,
        SaturateCompositeOp,
        ColorizeCompositeOp,
        LuminizeCompositeOp,
        ScreenCompositeOp,
        OverlayCompositeOp,
        CopyCyanCompositeOp,
        CopyMagentaCompositeOp,
        CopyYellowCompositeOp,
        CopyBlackCompositeOp,
        DivideCompositeOp,
        HardLightCompositeOp,
        ExclusionCompositeOp,
        ColorDodgeCompositeOp,
        ColorBurnCompositeOp,
        SoftLightCompositeOp,
        LinearBurnCompositeOp,
        LinearDodgeCompositeOp,
        LinearLightCompositeOp,
        VividLightCompositeOp,
        PinLightCompositeOp,
        HardMixCompositeOp,
    }
}

magick_enum! {
    FilterTypes,
    types::FilterTypes,
    u32,
    {
        UndefinedFilter,
        PointFilter,
        BoxFilter,
        TriangleFilter,
        HermiteFilter,
        HanningFilter,
        HammingFilter,
        BlackmanFilter,
        GaussianFilter,
        QuadraticFilter,
        CubicFilter,
        CatromFilter,
        MitchellFilter,
        LanczosFilter,
        BesselFilter,
        SincFilter,
    }
}

fn create_image_info<'a>(wand: &mut MagickWand<'a>) -> Result<ImageInfo> {
    Ok(ImageInfo {
        resolution: wand.get_image_resolution()?,
        images: wand.get_number_images(),
        format: wand.get_image_format()?,
    })
}

pub fn lib_image(state: &Lua, sender: Sender<LuaAsyncCallback>) -> Result<()> {
    let image = state.create_table()?;

    // image.from_data
    let sender2 = sender.clone();
    let from_data_fn = state.create_function(move |state, data: LuaString| {
        let data = data.as_bytes().to_owned();
        let sender3 = sender2.clone();

        let fut = create_lua_future!(
            state,
            sender2,
            (),
            tokio::task::spawn_blocking(move || {
                // Ensure the image is valid
                let mut wand = MagickWand::new();
                wand.read_image_blob(&data)?;
                let info = create_image_info(&mut wand)?;
                drop(wand);

                Ok(Image(Arc::new(ImageInner { data, info }), sender3))
            }),
            |_state, _data: (), res: Result<Result<Image>, JoinError>| { Ok(res??) }
        );

        Ok(fut)
    })?;
    image.set("from_data", from_data_fn)?;

    image.set("CHANNEL_TYPE", ChannelType::create_table(state)?)?;

    image.set(
        "COMPOSITE_OPERATOR",
        CompositeOperator::create_table(state)?,
    )?;

    image.set("FILTER_TYPES", FilterTypes::create_table(state)?)?;

    state.globals().set("image", image)?;

    Ok(())
}

#[derive(Clone)]
pub struct Image(Arc<ImageInner>, Sender<LuaAsyncCallback>);

impl Image {
    pub fn copy_data(&self) -> Vec<u8> {
        self.0.data.clone()
    }

    pub fn async_sender(&self) -> Sender<LuaAsyncCallback> {
        self.1.clone()
    }

    pub fn info(&self) -> &ImageInfo {
        &self.0.info
    }

    pub fn get_wand(&self) -> Result<MagickWand<'_>> {
        let mut wand = MagickWand::new();
        wand.read_image_blob(&self.0.data)?;
        Ok(wand)
    }
}

macro_rules! image_method {
    ($methods:expr, $name:expr, |$args:pat|: $args_ty:ty, |$wand:ident| $block:block) => {
        $methods.add_method($name, |state, image, $args: $args_ty| {
            let data = image.copy_data();
            let sender = image.async_sender();

            let fut = create_lua_future!(
                state,
                sender,
                (),
                tokio::task::spawn_blocking(move || {
                    let mut $wand = MagickWand::new();
                    $wand.read_image_blob(&data)?;

                    let wand = $block;

                    let data = wand
                        .write_image_blob()
                        .ok_or_else(|| anyhow::anyhow!("unable to write image"))?;

                    let info = create_image_info(wand)?;

                    Ok(Image(Arc::new(ImageInner { data, info }), sender))
                }),
                |_state, _data: (), res: Result<Result<Image>, JoinError>| { Ok(res??) }
            );

            Ok(fut)
        });
    };
}

impl UserData for Image {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        image_method!(methods, "blur", |(radius, sigma)|: (f64, f64), |wand| {
            wand.blur_image(radius, sigma)?
        });

        image_method!(methods, "charcoal", |(radius, sigma)|: (f64, f64), |wand| {
            wand.charcoal_image(radius, sigma)?
        });

        image_method!(methods, "chop", |(width, height, x, y)|: (u64, u64, i64, i64), |wand| {
            wand.chop_image(width, height, x, y)?
        });

        image_method!(methods, "colorize", |(colorize, opacity)|: (String, String), |wand| {
            wand.colorize_image(PixelWand::new().set_color(&colorize), PixelWand::new().set_color(&opacity))?
        });

        image_method!(methods, "composite", |(other_image, operator, x, y)|: (Image, CompositeOperator, i64, i64), |wand| {
            wand.composite_image(&other_image.get_wand()?, operator.inner(), x, y)?
        });

        image_method!(methods, "crop", |(width, height, x, y)|: (u64, u64, i64, i64), |wand| {
            wand.crop_image(width, height, x, y)?
        });

        // TODO: Draw method

        image_method!(methods, "extent", |(width, height, x, y)|: (u64, u64, i64, i64), |wand| {
            wand.extent_image(width, height, x, y)?
        });

        image_method!(methods, "flip", |(flip, flop)|: (bool, bool), |wand| {
            if flip && flop {
                wand.flip_image()?.flop_image()?
            } else if flip {
                wand.flip_image()?
            } else if flop {
                wand.flop_image()?
            } else {
                &mut wand
            }
        });

        image_method!(methods, "gamma", |gamma|: f64, |wand| {
            wand.gamma_image(gamma)?
        });

        image_method!(methods, "implode", |radius|: f64, |wand| {
            wand.implode_image(radius)?
        });

        image_method!(methods, "morph", |frames|: u64, |wand| {
            &mut wand.morph_images(frames)
        });

        image_method!(methods, "modulate", |(brightness, saturation, hue)|: (f64, f64, f64), |wand| {
            wand.modulate_image(brightness, saturation, hue)?
        });

        image_method!(methods, "negate", |gray|: u32, |wand| {
            wand.negate_image(gray)?
        });

        image_method!(methods, "oil_paint", |radius|: f64, |wand| {
            wand.oil_paint_image(radius)?
        });

        image_method!(methods, "opaque", |(target, fill, fuzz)|: (String, String, f64), |wand| {
            wand.opaque_image(PixelWand::new().set_color(&target), PixelWand::new().set_color(&fill), fuzz)?
        });

        image_method!(methods, "radial_blur", |angle|: f64, |wand| {
            wand.radial_blur_image(angle)?
        });

        image_method!(methods, "reduce_noise", |radius|: f64, |wand| {
            wand.reduce_noise_image(radius)?
        });

        image_method!(methods, "resample", |(x_resolution, y_resolution, filter, blur)|: (f64, f64, FilterTypes, f64), |wand| {
            wand.resample_image(x_resolution, y_resolution, filter.inner(), blur)?
        });

        image_method!(methods, "resize", |(columns, rows, filter, blur)|: (f64, f64, FilterTypes, f64), |wand| {
            wand.resample_image(columns, rows, filter.inner(), blur)?
        });

        image_method!(methods, "roll", |(x_offset, y_offset)|: (i64, i64), |wand| {
            wand.roll_image(x_offset, y_offset)?
        });

        image_method!(methods, "rotate", |(background, degress)|: (String, f64), |wand| {
            wand.rotate_image(PixelWand::new().set_color(&background), degress)?
        });

        image_method!(methods, "sample", |(columns, rows)|: (u64, u64), |wand| {
            wand.sample_image(columns, rows)?
        });

        image_method!(methods, "scale", |(columns, rows)|: (u64, u64), |wand| {
            wand.scale_image(columns, rows)?
        });

        image_method!(methods, "separate_channel", |channel|: ChannelType, |wand| {
            wand.separate_image_channel(channel.inner())?
        });

        image_method!(methods, "set_background", |background|: String, |wand| {
            wand.set_image_background_color(PixelWand::new().set_color(&background))?
        });

        image_method!(methods, "sharpen", |(radius, sigma)|: (f64, f64), |wand| {
            wand.sharpen_image(radius, sigma)?
        });

        image_method!(methods, "shave", |(columns, rows)|: (u64, u64), |wand| {
            wand.shave_image(columns, rows)?
        });

        image_method!(methods, "shear", |(background, x_shear, y_shear)|: (String, f64, f64), |wand| {
            wand.shear_image(PixelWand::new().set_color(&background), x_shear, y_shear)?
        });

        image_method!(methods, "solarize", |threshold|: f64, |wand| {
            wand.solarize_image(threshold)?
        });

        image_method!(methods, "spread", |radius|: f64, |wand| {
            wand.spread_image(radius)?
        });

        image_method!(methods, "swirl", |degrees|: f64, |wand| {
            wand.swirl_image(degrees)?
        });

        image_method!(methods, "texture", |texture_image|: Image, |wand| {
            &mut wand
                .texture_image(&texture_image.get_wand()?)
                .ok_or_else(|| anyhow::anyhow!("error texturing image"))?
        });

        image_method!(methods, "threshold", |threshold|: f64, |wand| {
            wand.threshold_image(threshold)?
        });

        image_method!(methods, "threshold_channel", |(channel, threshold)|: (ChannelType, f64), |wand| {
            wand.threshold_image_channel(channel.inner(), threshold)?
        });

        image_method!(methods, "tint", |(tint, opacity)|: (String, String), |wand| {
            wand.tint_image(PixelWand::new().set_color(&tint), PixelWand::new().set_color(&opacity))?
        });

        image_method!(methods, "transform", |(crop, geometry)|: (String, String), |wand| {
            &mut wand
                .transform_image(&crop, &geometry)
                .ok_or_else(|| anyhow::anyhow!("error transforming image"))?
        });

        image_method!(methods, "transparent", |(target, opacity, fuzz)|: (String, u8, f64), |wand| {
            wand.transparent_image(PixelWand::new().set_color(&target), opacity, fuzz)?
        });

        image_method!(methods, "trim", |fuzz|: f64, |wand| {
            wand.trim_image(fuzz)?
        });

        image_method!(methods, "unsharp_mask", |(radius, sigma, amount, threshold)|: (f64, f64, f64, f64), |wand| {
            wand.unsharp_mask_image(radius, sigma, amount, threshold)?
        });

        image_method!(methods, "wave", |(amplitude, wave_length)|: (f64, f64), |wand| {
            wand.wave_image(amplitude, wave_length)?
        });

        methods.add_meta_method(MetaMethod::Index, |state, image, index: String| match index
            .as_str()
        {
            "size" => Ok(mlua::Value::Number(image.0.data.len() as f64)),
            "width" => Ok(mlua::Value::Number(image.info().resolution.0)),
            "height" => Ok(mlua::Value::Number(image.info().resolution.1)),
            "images" => Ok(mlua::Value::Number(image.info().images as f64)),
            "format" => Ok(mlua::Value::String(
                state.create_string(&image.info().format)?,
            )),
            "data" => Ok(mlua::Value::String(state.create_string(&image.0.data)?)),
            _ => Ok(mlua::Value::Nil),
        });

        methods.add_meta_method(MetaMethod::ToString, |state, image, (): ()| {
            state.create_string(&format!(
                "Image {{ width = {}, height = {}, format = \"{}\" }}",
                image.info().resolution.0,
                image.info().resolution.1,
                image.info().format
            ))
        });
    }
}

pub struct ImageInner {
    data: Vec<u8>,
    info: ImageInfo,
}

pub struct ImageInfo {
    resolution: (f64, f64),
    images: u64,
    format: String,
}
