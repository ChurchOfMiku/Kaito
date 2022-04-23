use anyhow::Result;
use crossbeam::channel::Sender;
use futures::TryStreamExt;
use graphicsmagick::{
    types,
    wand::{DrawingWand, MagickWand, PixelWand},
};
use hyper::{Body, Client, Request};
use hyper_tls::HttpsConnector;
use mlua::{
    prelude::{FromLua, LuaError, LuaMultiValue, LuaString, LuaTable, LuaValue},
    Lua, MetaMethod, UserData, UserDataMethods,
};
use std::{
    net::IpAddr,
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::task::JoinError;

use crate::{
    bot::Bot,
    modules::lua::{
        http::HttpError,
        lib::bot::BotMessage,
        state::{get_sandbox_state, LuaAsyncCallback},
    },
    services::{Message, ServiceKind},
};

const MAX_IMAGE_SIZE: usize = 1024 * 1024 * 4; // Max 4MB
const IMAGE_EXTENSIONS: &[&'static str] = &["png", "gif", "jpg", "jpeg"];

macro_rules! magick_enum {
    ($name:ident, $inner:ty, $num_ty:ty, { $($lua_ident:ident => $enum_ident:ident,)+ }) => {
        pub struct $name($inner);

        impl $name {
            pub fn create_table(state: &Lua) -> Result<LuaTable> {
                let tbl = state.create_table()?;

                $(
                    tbl.set(stringify!($lua_ident), <$inner>::$enum_ident as $num_ty)?;
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
        Undefined => UndefinedChannel,
        Red => RedChannel,
        Cyan => CyanChannel,
        Green => GreenChannel,
        Magenta => MagentaChannel,
        Blue => BlueChannel,
        Yellow => YellowChannel,
        Opacity => OpacityChannel,
        Black => BlackChannel,
        Matte => MatteChannel,
        All =>AllChannels,
        Gray => GrayChannel,
    }
}

magick_enum! {
    CompositeOperator,
    types::CompositeOperator,
    u32,
    {
        Undefined => UndefinedCompositeOp,
        Over => OverCompositeOp,
        In => InCompositeOp,
        Out => OutCompositeOp,
        Atop => AtopCompositeOp,
        Xor => XorCompositeOp,
        Plus => PlusCompositeOp,
        Minus => MinusCompositeOp,
        Add => AddCompositeOp,
        Subtract => SubtractCompositeOp,
        Difference => DifferenceCompositeOp,
        Multiply => MultiplyCompositeOp,
        Bumpmap => BumpmapCompositeOp,
        Copy => CopyCompositeOp,
        CopyRed => CopyRedCompositeOp,
        CopyGreen => CopyGreenCompositeOp,
        CopyBlue => CopyBlueCompositeOp,
        CopyOpacity => CopyOpacityCompositeOp,
        Clear => ClearCompositeOp,
        Dissolve => DissolveCompositeOp,
        Displace => DisplaceCompositeOp,
        Modulate => ModulateCompositeOp,
        Threshold => ThresholdCompositeOp,
        No => NoCompositeOp,
        Darken => DarkenCompositeOp,
        Lighten => LightenCompositeOp,
        Hue => HueCompositeOp,
        Saturate => SaturateCompositeOp,
        Colorize => ColorizeCompositeOp,
        Lumenize => LuminizeCompositeOp,
        Screen => ScreenCompositeOp,
        Overlay => OverlayCompositeOp,
        CopyCyan => CopyCyanCompositeOp,
        CopyMagenta => CopyMagentaCompositeOp,
        CopyYellow => CopyYellowCompositeOp,
        CopyBlack => CopyBlackCompositeOp,
        Divide => DivideCompositeOp,
        HardLight => HardLightCompositeOp,
        Exclusion => ExclusionCompositeOp,
        ColorDodge => ColorDodgeCompositeOp,
        ColorBurn => ColorBurnCompositeOp,
        SoftLight => SoftLightCompositeOp,
        LinearBurn => LinearBurnCompositeOp,
        LinearDodge => LinearDodgeCompositeOp,
        LinearLight => LinearLightCompositeOp,
        VividLight => VividLightCompositeOp,
        PinLight => PinLightCompositeOp,
        HardMix => HardMixCompositeOp,
    }
}

magick_enum! {
    DecorationType,
    types::DecorationType,
    u32,
    {
        None => NoDecoration,
        Underline => UnderlineDecoration,
        Overline => OverlineDecoration,
        Strikethrough => LineThroughDecoration,
    }
}

magick_enum! {
    FilterTypes,
    types::FilterTypes,
    u32,
    {
        Undefined => UndefinedFilter,
        Point => PointFilter,
        Box => BoxFilter,
        Triangle => TriangleFilter,
        Hermite => HermiteFilter,
        Hanning => HanningFilter,
        Hamming => HammingFilter,
        Blackman => BlackmanFilter,
        Gaussian => GaussianFilter,
        Quadratic => QuadraticFilter,
        Cubic => CubicFilter,
        Catrom => CatromFilter,
        Mitchell => MitchellFilter,
        Lanczos => LanczosFilter,
        Bessel => BesselFilter,
        Sinc => SincFilter,
    }
}

magick_enum! {
    LineCap,
    types::LineCap,
    u32,
    {
        Undefined => UndefinedCap,
        Butt => ButtCap,
        Round => RoundCap,
        Square => SquareCap,
    }
}

magick_enum! {
    LineJoin,
    types::LineJoin,
    u32,
    {
        Undefined => UndefinedJoin,
        Miter => MiterJoin,
        Round => RoundJoin,
        Bevel => BevelJoin,
    }
}

magick_enum! {
    PaintMethod,
    types::PaintMethod,
    u32,
    {
        Point => PointMethod,
        Replace => ReplaceMethod,
        Floodfill => FloodfillMethod,
        FillToBorder =>FillToBorderMethod,
        Reset => ResetMethod,
    }
}

magick_enum! {
    FillRule,
    types::FillRule,
    u32,
    {
        Undefined => UndefinedRule,
        EvenOdd => EvenOddRule,
        NonZero => NonZeroRule,
    }
}

magick_enum! {
    StyleType,
    types::StyleType,
    u32,
    {
        Normal => NormalStyle,
        Italic => ItalicStyle,
        Oblique => ObliqueStyle,
        Any => AnyStyle,
    }
}

magick_enum! {
    TextAlign,
    types::GravityType,
    u32,
    {
        Forget => ForgetGravity,
        TopLeft => NorthWestGravity,
        TopCenter => NorthGravity,
        TopRight => NorthEastGravity,
        LeftCenter => WestGravity,
        Center => CenterGravity,
        RightCenter => EastGravity,
        LeftBottom => SouthWestGravity,
        BottomCenter => SouthGravity,
        BottomRight => SouthEastGravity,
        Static => StaticGravity,
    }
}

fn create_image_info<'a>(wand: &mut MagickWand<'a>) -> Result<ImageInfo> {
    Ok(ImageInfo {
        width: wand.get_image_width(),
        height: wand.get_image_height(),
        images: wand.get_number_images(),
        format: wand.get_image_format()?.to_lowercase(),
    })
}

fn check_url(url: &url::Url) -> Result<String> {
    match url.scheme() {
        "http" | "https" => {}
        s => {
            return Err(anyhow::anyhow!("unknown scheme: {}", s));
        }
    }

    let addrs = match url.socket_addrs(|| Some(if url.scheme() == "https" { 443 } else { 80 })) {
        Ok(addrs) => addrs,
        Err(err) => {
            return Err(anyhow::anyhow!("error resolving hosts: {}", err));
        }
    };

    let disallowed_addr = addrs.iter().find(|addr| {
        let ip = addr.ip();
        ip.is_loopback()
            || ip.is_multicast()
            || ip.is_unspecified()
            || (match ip {
                IpAddr::V4(ip) => match ip.octets() {
                    [10, ..] => true,
                    [172, b, ..] if b >= 16 && b <= 31 => true,
                    [192, 168, ..] => true,
                    _ => false,
                },
                IpAddr::V6(_) => false, // IPv6 should be disabled in networking
            })
    });

    if let Some(disallowed_addr) = disallowed_addr {
        return Err(anyhow::anyhow!(
            "disallowed ip address found: {}",
            disallowed_addr
        ));
    }

    Ok(url.to_string())
}

async fn download_image(url: &url::Url) -> Result<Vec<u8>> {
    let client = Client::builder().build::<_, Body>(HttpsConnector::new());
    let req = Request::builder()
        .method("GET")
        .uri(check_url(&url)?)
        .body(Body::empty())?;
    let mut res = client.request(req).await?;

    let body = res
        .body_mut()
        .map_err(|e: hyper::Error| e.into())
        .try_fold(Vec::new(), |mut data, chunk| async move {
            data.extend_from_slice(&chunk);

            if data.len() > MAX_IMAGE_SIZE {
                return Err(anyhow::anyhow!("max body size limit reached")).into();
            }

            Ok(data)
        })
        .await?;

    Ok(body)
}

async fn create_image(sender: Sender<LuaAsyncCallback>, data: Vec<u8>, svg: bool) -> Result<Image> {
    match tokio::task::spawn_blocking(move || {
        // Ensure the image is valid
        let mut wand = MagickWand::new();

        if svg {
            wand.set_size(1024, 1024)?;
            wand.set_format("SVG")?;
        }

        wand.read_image_blob(&data)?;

        let (data, info) = if svg {
            wand.set_image_format("PNG")?;
            wand.transparent_image(&PixelWand::new().set_color("white"), 255, 32.0)?;
            wand.resize_image(512, 512, types::FilterTypes::LanczosFilter, 1.0)?;
            wand.trim_image(1.0)?;
            let data = wand
                .write_image_blob()
                .ok_or_else(|| anyhow::anyhow!("unable to convert svg"))?;

            let info = create_image_info(&mut wand)?;
            drop(wand);

            (data, info)
        } else {
            let info = create_image_info(&mut wand)?;
            drop(wand);

            (data, info)
        };

        Ok(Image(Arc::new(ImageInner { data, info }), sender))
    })
    .await
    {
        Ok(res) => res,
        Err(err) => Err(err.into()),
    }
}

pub fn lib_image(state: &Lua, bot: Arc<Bot>, sender: Sender<LuaAsyncCallback>) -> Result<()> {
    let image = state.create_table()?;

    // image.create_draw_buffer
    let create_draw_buffer_fn =
        state.create_function(|_, (): ()| Ok(DrawCommandBuffer::default()))?;
    image.set("create_draw_buffer", create_draw_buffer_fn)?;

    // image.create_path
    let create_path_fn = state.create_function(|_, (): ()| Ok(PathCommandBuffer::default()))?;
    image.set("create_path", create_path_fn)?;

    // image.create
    let sender2 = sender.clone();
    let create_image_fn = state.create_function(
        move |_, (width, height, background): (u64, u64, Option<String>)| {
            if width > 2000 || height > 2000 {
                return Err(LuaError::RuntimeError(
                    "image cannot be bigger than 2000x2000".into(),
                ));
            }

            let mut wand = MagickWand::new();

            wand.set_size(width, height)
                .map_err(|e| LuaError::RuntimeError(e.to_string()))?;
            wand.read_image("xc:none")
                .map_err(|e| LuaError::RuntimeError(e.to_string()))?;
            wand.set_image_format("PNG")
                .map_err(|e| LuaError::RuntimeError(e.to_string()))?;

            if let Some(background) = background {
                let mut draw = DrawingWand::new();

                wand.draw_image(
                    &draw
                        .set_fill_color(&PixelWand::new().set_color(&background))
                        .set_stroke_line_cap(types::LineCap::SquareCap)
                        .rectangle(-8.0, -8.0, (width + 8) as f64, (height + 8) as f64),
                )
                .map_err(|e| LuaError::RuntimeError(e.to_string()))?;
            }

            let data = wand
                .write_image_blob()
                .ok_or_else(|| LuaError::RuntimeError("unable to write image".into()))?;
            let info =
                create_image_info(&mut wand).map_err(|e| LuaError::RuntimeError(e.to_string()))?;
            drop(wand);

            Ok(Image(Arc::new(ImageInner { data, info }), sender2.clone()))
        },
    )?;
    image.set("create", create_image_fn)?;

    // image.from_data
    let sender2 = sender.clone();
    let from_data_fn = state.create_function(move |state, data: LuaString| {
        if let Some(sandbox_state) = get_sandbox_state(state) {
            if sandbox_state.limits().images_left_limit() {
                return Err(LuaError::RuntimeError("image limit reached".into()));
            }
        }

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

    // image.from_url
    let sender2 = sender.clone();
    let from_url_fn = state.create_function(move |state, url: String| {
        if let Some(sandbox_state) = get_sandbox_state(state) {
            if sandbox_state.limits().images_left_limit() {
                return Err(LuaError::RuntimeError("image limit reached".into()));
            }
        }

        let sender3 = sender2.clone();

        // Parse url
        let (url, svg) = match url::Url::parse(&url) {
            Ok(url) => (
                check_url(&url).map_err(|err| LuaError::RuntimeError(err.to_string()))?,
                url.path().ends_with(".svg"),
            ),
            Err(err) => {
                return Err(LuaError::ExternalError(Arc::new(
                    HttpError::ErrorParsingUrl(err.to_string()),
                )));
            }
        };

        let https = HttpsConnector::new();
        let client = Client::builder().build::<_, Body>(https);

        let req = match Request::builder()
            .method("GET")
            .uri(url.clone())
            .body(Body::empty())
        {
            Ok(req) => req,
            Err(err) => {
                return Err(LuaError::ExternalError(Arc::new(
                    HttpError::ErrorBuildingRequest(err.to_string()),
                )));
            }
        };

        let fut = create_lua_future!(
            state,
            sender2,
            (),
            async move {
                match client.request(req).await {
                    Ok(mut res) => {
                        let body = res
                            .body_mut()
                            .map_err(|e: hyper::Error| e.into())
                            .try_fold(Vec::new(), |mut data, chunk| async move {
                                data.extend_from_slice(&chunk);

                                if data.len() > MAX_IMAGE_SIZE {
                                    return Err(anyhow::anyhow!("max body size limit reached"))
                                        .into();
                                }

                                Ok(data)
                            })
                            .await?;

                        create_image(sender3, body, svg).await
                    }
                    Err(err) => Err(err.into()),
                }
            },
            |_state, _data: (), res: Result<Image>| { Ok(res?) }
        );

        Ok(fut)
    })?;
    image.set("from_url", from_url_fn)?;

    // image.resolve
    let sender2 = sender.clone();
    let resolve_fn = state.create_function(
        move |state, (msg, text): (Option<BotMessage>, Option<String>)| {
            if let Some(sandbox_state) = get_sandbox_state(state) {
                if sandbox_state.limits().images_left_limit() {
                    return Err(LuaError::RuntimeError("image limit reached".into()));
                }
            }

            let sender3 = sender2.clone();
            let bot = bot.clone();
            let fut = create_lua_future!(
                state,
                sender2,
                (),
                async move {
                    if let Some(text) = text {
                        let text = text.trim();
                        if !text.is_empty() {
                            if text.starts_with("https://") || text.starts_with("http://") {
                                // Check if it is an url
                                match url::Url::parse(text) {
                                    Ok(url) => {
                                        return Ok(Some(
                                            create_image(sender3, download_image(&url).await?, url.path().ends_with(".svg"))
                                                .await?,
                                        ));
                                    }
                                    Err(_) => {}
                                };
                            }

                            if let Some(msg) = msg.as_ref() {
                                if msg.service_kind() == ServiceKind::Discord {
                                    lazy_static::lazy_static! {
                                        static ref RE: regex::Regex = regex::Regex::new(r#"<a:.+?:(\d+)>|<:.+?:(\d+)>"#).unwrap();
                                    }

                                    if let Some(capture) = RE.captures(&text) {
                                        if let Some(url) = if let Some(animated_id) = capture.get(1) {
                                            Some(format!("https://cdn.discordapp.com/emojis/{}.gif?v=1", animated_id.as_str()))
                                        } else if let Some(id) = capture.get(2) {
                                            Some(format!("https://cdn.discordapp.com/emojis/{}.png?v=1", id.as_str()))
                                        } else {
                                            None
                                        } {
                                            if let Ok(image_data) = download_image(&url::Url::parse(&url)?).await {
                                                return Ok(Some(create_image(sender3, image_data, false).await?));
                                            }
                                        }
                                    }
                                }

                                if let Some(emoji) = emojis::get(text) {
                                    let code = emoji.as_str().chars().map(|code| format!("{:x}", code as u32)).collect::<Vec<_>>().join("-");
                                    let url = format!("https://cdnjs.cloudflare.com/ajax/libs/twemoji/13.0.2/svg/{}.svg", code);
                                    if let Ok(image_data) = download_image(&url::Url::parse(&url)?).await {
                                        return Ok(Some(create_image(sender3, image_data, true).await?));
                                    }
                                }

                                if let Ok(user) = bot
                                    .get_ctx()
                                    .services()
                                    .find_user(msg.channel().id(), text)
                                    .await
                                {
                                    if let Some(avatar) = user.avatar() {
                                        return Ok(Some(
                                            create_image(
                                                sender3,
                                                download_image(&url::Url::parse(avatar.trim())?)
                                                    .await?, false,
                                            )
                                            .await?,
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    if let Some(msg) = msg {
                        for attachment in msg.attachments() {
                            if let Some(extension) = Path::new(&attachment.filename).extension() {
                                if IMAGE_EXTENSIONS.contains(&&*extension.to_string_lossy()) {
                                    return Ok(Some(
                                        create_image(
                                            sender3,
                                            download_image(&url::Url::parse(&attachment.url)?)
                                                .await?, attachment.filename.ends_with(".svg")
                                        )
                                        .await?,
                                    ));
                                }
                            }
                        }

                        let id = msg.channel().id();
                        let channel = bot.get_ctx().services().channel(id).await?;
                        if let Ok(messages) = channel.messages(16, None).await {
                            for message in messages {
                                for attachment in message.attachments() {
                                    if let Some(extension) =
                                        Path::new(&attachment.filename).extension()
                                    {
                                        if IMAGE_EXTENSIONS.contains(&&*extension.to_string_lossy())
                                        {
                                            return Ok(Some(
                                                create_image(
                                                    sender3,
                                                    download_image(&url::Url::parse(
                                                        &attachment.url,
                                                    )?)
                                                    .await?,
                                                    attachment.filename.ends_with(".svg")
                                                )
                                                .await?,
                                            ));
                                        }
                                    }
                                }

                                let text = message.content().trim();

                                if !text.is_empty()
                                    && (text.starts_with("https://") || text.starts_with("http://"))
                                {
                                    match url::Url::parse(text) {
                                        Ok(url) => {
                                            return Ok(Some(
                                                create_image(sender3, download_image(&url).await?, url.path().ends_with(".svg"))
                                                    .await?,
                                            ));
                                        }
                                        Err(_) => {}
                                    };
                                }
                            }
                        }
                    }

                    Ok(None)
                },
                |_state, _data: (), res: Result<Option<Image>>| { Ok(res?) }
            );

            Ok(fut)
        },
    )?;
    image.set("resolve", resolve_fn)?;

    image.set("CHANNEL_TYPE", ChannelType::create_table(state)?)?;
    image.set(
        "COMPOSITE_OPERATOR",
        CompositeOperator::create_table(state)?,
    )?;
    image.set("FILL_RULE", FillRule::create_table(state)?)?;
    image.set("FONT_STYLE", StyleType::create_table(state)?)?;
    image.set("FILTER_TYPES", FilterTypes::create_table(state)?)?;
    image.set("LINE_CAP", LineCap::create_table(state)?)?;
    image.set("LINE_JOIN", LineJoin::create_table(state)?)?;
    image.set("PAINT_METHOD", PaintMethod::create_table(state)?)?;
    image.set("TEXT_ALIGN", TextAlign::create_table(state)?)?;
    image.set("TEXT_DECORATION", DecorationType::create_table(state)?)?;

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
    ($methods:expr, $name:expr, $all_frames:tt, |$args:pat|: $args_ty:ty, |$wand:ident| $block:block) => {
        $methods.add_method($name, |state, image, $args: $args_ty| {
            if let Some(sandbox_state) = get_sandbox_state(state) {
                if sandbox_state.limits().image_operations_left_limit() {
                    return Err(LuaError::RuntimeError(
                        "image operation limit reached".into(),
                    ));
                }
            }

            let data = image.copy_data();
            let sender = image.async_sender();

            let fut = create_lua_future!(
                state,
                sender,
                (),
                tokio::task::spawn_blocking(move || {
                    let mut $wand = MagickWand::new();
                    $wand.read_image_blob(&data)?;

                    let mut data = Vec::new();
                    let wand = image_method!(__frames, $block, $wand, $all_frames);

                    wand.reset_iterator();

                    for _ in 0..wand.get_number_images() {
                        data.append(
                            &mut wand
                                .write_image_blob()
                                .ok_or_else(|| anyhow::anyhow!("unable to write image"))?,
                        );
                    }

                    let info = create_image_info(wand)?;

                    Ok(Image(Arc::new(ImageInner { data, info }), sender))
                }),
                |_state, _data: (), res: Result<Result<Image>, JoinError>| { Ok(res??) }
            );

            Ok(fut)
        });
    };
    (__frames, $block:block, $wand:ident, true) => {{
        $wand.reset_iterator();

        for _ in 0..=$wand.get_number_images() {
            $block;
            $wand.next_image();
        }

        &mut $wand
    }};
    (__frames, $block:block, $wand:expr, false) => {
        &mut $block
    };
}

impl UserData for Image {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        image_method!(methods, "blur", true, |(radius, sigma)|: (f64, f64), |wand| {
            wand.blur_image(radius, sigma)?
        });

        image_method!(methods, "charcoal", true, |(radius, sigma)|: (f64, f64), |wand| {
            wand.charcoal_image(radius, sigma)?
        });

        image_method!(methods, "chop", true, |(width, height, x, y)|: (u64, u64, i64, i64), |wand| {
            wand.chop_image(width, height, x, y)?
        });

        image_method!(methods, "colorize", true, |(colorize, opacity)|: (String, String), |wand| {
            wand.colorize_image(PixelWand::new().set_color(&colorize), PixelWand::new().set_color(&opacity))?
        });

        image_method!(methods, "composite", true, |(other_image, operator, x, y)|: (Image, CompositeOperator, i64, i64), |wand| {
            wand.composite_image(&other_image.get_wand()?, operator.inner(), x, y)?
        });

        image_method!(methods, "crop", true, |(width, height, x, y)|: (u64, u64, i64, i64), |wand| {
            wand.crop_image(width, height, x, y)?
        });

        image_method!(methods, "draw", true, |draw_commands|: DrawCommandBuffer, |wand| {
            let mut draw_wand = &mut DrawingWand::new();

            for command in draw_commands.commands.lock().unwrap().drain(..) {
                draw_wand = match command {
                    DrawCommand::Arc { sx, sy, ex, ey, sd, ed } => draw_wand.arc(sx, sy, ex, ey, sd, ed),
                    DrawCommand::Circle { ox, oy, px, py } => draw_wand.circle(ox, oy, px, py),
                    DrawCommand::Color { x, y, paint_method } => draw_wand.color(x, y, paint_method.inner()),
                    DrawCommand::Ellipse { ox, oy, rx, ry, start, end } => draw_wand.ellipse(ox, oy, rx, ry, start, end),
                    DrawCommand::SetFillColor { color } => draw_wand.set_fill_color(PixelWand::new().set_color(&color)),
                    DrawCommand::SetFillOpacity(opacity) => draw_wand.set_fill_opacity(opacity),
                    DrawCommand::SetFillRule(rule) => draw_wand.set_fill_rule(rule.inner()),
                    DrawCommand::SetFont(font) => draw_wand.set_font(&font),
                    DrawCommand::SetFontFamily(font_family) => draw_wand.set_font_family(&font_family),
                    DrawCommand::SetFontSize(font_size) => draw_wand.set_font_size(font_size),
                    DrawCommand::SetFontStyle(font_style) => draw_wand.set_font_style(font_style.inner()),
                    DrawCommand::Line { sx, sy, ex, ey } => draw_wand.line(sx, sy, ex, ey),
                    DrawCommand::Matte { x, y, paint_method } => draw_wand.matte(x, y, paint_method.inner()),
                    DrawCommand::Path(commands) => {
                        draw_wand = draw_wand.path_start();

                        for command in commands {
                            draw_wand = match command {
                                PathCommand::Close => draw_wand.path_close(),
                                PathCommand::Absolute { x1, y1, x2, y2, x, y } => draw_wand.path_curve_to_absolute(x1, y1, x2, y2, x, y),
                                PathCommand::Relative { x1, y1, x2, y2, x, y } => draw_wand.path_curve_to_relative(x1, y1, x2, y2, x, y),
                                PathCommand::QuadraticBezierAbsolute { x1, y1, x, y } => draw_wand.path_curve_to_quadratic_bezier_absolute(x1, y1, x, y),
                                PathCommand::QuadraticBezierRelative { x1, y1, x, y } => draw_wand.path_curve_to_quadratic_bezier_relative(x1, y1, x, y),
                                PathCommand::QuadraticBezierSmoothAbsolute { x, y } => draw_wand.path_curve_to_quadratic_bezier_smooth_absolute(x, y),
                                PathCommand::QuadraticBezierSmoothRelative { x, y } => draw_wand.path_curve_to_quadratic_bezier_smooth_relative(x, y),
                                PathCommand::SmoothAbsolute { x2, y2, x, y } => draw_wand.path_curve_to_smooth_absolute(x2, y2, x, y),
                                PathCommand::SmoothRelative { x2, y2, x, y } => draw_wand.path_curve_to_smooth_relative(x2, y2, x, y),
                                PathCommand::EllipticArcAbsolute { rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, x, y } => draw_wand.path_elliptic_arc_absolute(rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, x, y),
                                PathCommand::EllipticArcRelative { rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, x, y } => draw_wand.path_elliptic_arc_relative(rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, x, y),
                                PathCommand::LineAbsolute { x, y } => draw_wand.path_line_to_absolute(x, y),
                                PathCommand::LineRelative { x, y } => draw_wand.path_line_to_relative(x, y),
                                PathCommand::LineHorizontalAbsolute(x) => draw_wand.path_line_to_horizontal_absolute(x),
                                PathCommand::LineHorizontalRelative(x) => draw_wand.path_line_to_horizontal_relative(x),
                                PathCommand::LineVerticalAbsolute(y) => draw_wand.path_line_to_vertical_absolute(y),
                                PathCommand::LineVerticalRelative(y) => draw_wand.path_line_to_vertical_absolute(y),
                                PathCommand::MoveToAbsolute { x, y } => draw_wand.path_move_to_absolute(x, y),
                                PathCommand::MoveToRelative { x, y } => draw_wand.path_move_to_relative(x, y)
                            }
                        }

                        draw_wand.path_finish()
                    },
                    DrawCommand::Rectangle { x1, y1, x2, y2 } => draw_wand.rectangle(x1, y1, x2, y2),
                    DrawCommand::Rotate(degrees) => draw_wand.rotate(degrees),
                    DrawCommand::RoundRectangle { x1, y1, x2, y2, rx, ry } => draw_wand.round_rectangle(x1, y1, x2, y2, rx, ry),
                    DrawCommand::Scale { x, y } => draw_wand.scale(x, y),
                    DrawCommand::SkewX(degrees) => draw_wand.skew_x(degrees),
                    DrawCommand::SkewY(degrees) => draw_wand.skew_y(degrees),
                    DrawCommand::SetStrokeAntiAlias(aa) => draw_wand.set_stroke_antialias(aa),
                    DrawCommand::SetStrokeColor(color) => draw_wand.set_stroke_color(PixelWand::new().set_color(&color)),
                    DrawCommand::SetStrokeLineCap(cap) => draw_wand.set_stroke_line_cap(cap.inner()),
                    DrawCommand::SetStrokeLineJoin(join) => draw_wand.set_stroke_line_join(join.inner()),
                    DrawCommand::SetStrokeWidth(width) => draw_wand.set_stroke_width(width),
                    DrawCommand::SetTextAntiAlias(aa) => draw_wand.set_text_antialias(aa),
                    DrawCommand::SetTextDecoration(decoration) => draw_wand.set_text_decoration(decoration.inner()),
                    DrawCommand::SetTextUnderColor(color) => draw_wand.set_text_under_color(PixelWand::new().set_color(&color)),
                    DrawCommand::Text { align, x, y, text } => draw_wand.set_gravity(align.inner()).annotation(x, y, &text),
                    DrawCommand::Translate { x, y } => draw_wand.translate(x, y),
                    DrawCommand::SetViewbox { x1, y1, x2, y2 } => draw_wand.set_viewbox(x1, y1, x2, y2),
                }
            }

            wand.draw_image(&draw_wand)?
        });

        image_method!(methods, "extent", true, |(width, height, x, y)|: (u64, u64, i64, i64), |wand| {
            wand.extent_image(width, height, x, y)?
        });

        image_method!(methods, "flip", true, |(flip, flop)|: (bool, bool), |wand| {
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

        image_method!(methods, "gamma", true, |gamma|: f64, |wand| {
            wand.gamma_image(gamma)?
        });

        image_method!(methods, "implode", true, |radius|: f64, |wand| {
            wand.implode_image(radius)?
        });

        image_method!(methods, "morph", false, |frames|: u64, |wand| {
            wand.morph_images(frames)
        });

        image_method!(methods, "modulate", true, |(brightness, saturation, hue)|: (f64, f64, f64), |wand| {
            wand.modulate_image(brightness, saturation, hue)?
        });

        image_method!(methods, "negate", true, |gray|: u32, |wand| {
            wand.negate_image(gray)?
        });

        image_method!(methods, "oil_paint", true, |radius|: f64, |wand| {
            wand.oil_paint_image(radius)?
        });

        image_method!(methods, "opaque", true, |(target, fill, fuzz)|: (String, String, f64), |wand| {
            wand.opaque_image(PixelWand::new().set_color(&target), PixelWand::new().set_color(&fill), fuzz)?
        });

        image_method!(methods, "radial_blur", true, |angle|: f64, |wand| {
            wand.radial_blur_image(angle)?
        });

        image_method!(methods, "reduce_noise", true, |radius|: f64, |wand| {
            wand.reduce_noise_image(radius)?
        });

        image_method!(methods, "resample", true, |(x_resolution, y_resolution, filter, blur)|: (f64, f64, FilterTypes, f64), |wand| {
            wand.resample_image(x_resolution, y_resolution, filter.inner(), blur)?
        });

        image_method!(methods, "resize", true, |(columns, rows, filter, blur)|: (u64, u64, FilterTypes, f64), |wand| {
            wand.resize_image(columns, rows, filter.inner(), blur)?
        });

        image_method!(methods, "roll", true, |(x_offset, y_offset)|: (i64, i64), |wand| {
            wand.roll_image(x_offset, y_offset)?
        });

        image_method!(methods, "rotate", true, |(background, degress)|: (String, f64), |wand| {
            wand.rotate_image(PixelWand::new().set_color(&background), degress)?
        });

        image_method!(methods, "sample", true, |(columns, rows)|: (u64, u64), |wand| {
            wand.sample_image(columns, rows)?
        });

        image_method!(methods, "scale", true, |(columns, rows)|: (u64, u64), |wand| {
            wand.scale_image(columns, rows)?
        });

        image_method!(methods, "separate_channel", true, |channel|: ChannelType, |wand| {
            wand.separate_image_channel(channel.inner())?
        });

        image_method!(methods, "set_background", true, |background|: String, |wand| {
            wand.set_image_background_color(PixelWand::new().set_color(&background))?
        });

        image_method!(methods, "sharpen", true, |(radius, sigma)|: (f64, f64), |wand| {
            wand.sharpen_image(radius, sigma)?
        });

        image_method!(methods, "shave", true, |(columns, rows)|: (u64, u64), |wand| {
            wand.shave_image(columns, rows)?
        });

        image_method!(methods, "shear", true, |(background, x_shear, y_shear)|: (String, f64, f64), |wand| {
            wand.shear_image(PixelWand::new().set_color(&background), x_shear, y_shear)?
        });

        image_method!(methods, "solarize", true, |threshold|: f64, |wand| {
            wand.solarize_image(threshold)?
        });

        image_method!(methods, "spread", true, |radius|: f64, |wand| {
            wand.spread_image(radius)?
        });

        image_method!(methods, "swirl", true, |degrees|: f64, |wand| {
            wand.swirl_image(degrees)?
        });

        image_method!(methods, "texture", false, |texture_image|: Image, |wand| {
            wand
                .texture_image(&texture_image.get_wand()?)
                .ok_or_else(|| anyhow::anyhow!("error texturing image"))?
        });

        image_method!(methods, "threshold", true, |threshold|: f64, |wand| {
            wand.threshold_image(threshold)?
        });

        image_method!(methods, "threshold_channel", true, |(channel, threshold)|: (ChannelType, f64), |wand| {
            wand.threshold_image_channel(channel.inner(), threshold)?
        });

        image_method!(methods, "tint", true, |(tint, opacity)|: (String, String), |wand| {
            wand.tint_image(PixelWand::new().set_color(&tint), PixelWand::new().set_color(&opacity))?
        });

        image_method!(methods, "transform", false, |(crop, geometry)|: (String, String), |wand| {
            wand
                .transform_image(&crop, &geometry)
                .ok_or_else(|| anyhow::anyhow!("error transforming image"))?
        });

        image_method!(methods, "transparent", true, |(target, opacity, fuzz)|: (String, u8, f64), |wand| {
            wand.transparent_image(PixelWand::new().set_color(&target), opacity, fuzz)?
        });

        image_method!(methods, "trim", true, |trim|: f64, |wand| {
            wand.trim_image(trim)?
        });

        image_method!(methods, "unsharp_mask", true, |(radius, sigma, amount, threshold)|: (f64, f64, f64, f64), |wand| {
            wand.unsharp_mask_image(radius, sigma, amount, threshold)?
        });

        image_method!(methods, "wave", true, |(amplitude, wave_length)|: (f64, f64), |wand| {
            wand.wave_image(amplitude, wave_length)?
        });

        methods.add_meta_method(MetaMethod::Index, |state, image, index: String| match index
            .as_str()
        {
            "size" => Ok(mlua::Value::Number(image.0.data.len() as f64)),
            "width" => Ok(mlua::Value::Number(image.info().width as f64)),
            "height" => Ok(mlua::Value::Number(image.info().height as f64)),
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
                image.info().width,
                image.info().height,
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
    width: u64,
    height: u64,
    images: u64,
    format: String,
}

#[derive(Clone, Default)]
struct DrawCommandBuffer {
    commands: Arc<Mutex<Vec<DrawCommand>>>,
}

macro_rules! draw_method {
    ($methods:expr, $name:expr, |$args:pat|: $args_ty:ty, $block:block) => {
        $methods.add_method($name, |_state, buffer, $args: $args_ty| {
            let mut commands = buffer.commands.lock().unwrap();

            if commands.len() >= 32 {
                return Err(LuaError::RuntimeError(
                    "cannot have more than 32 draw commands in one draw command buffer".into(),
                ));
            }

            commands.push($block);
            Ok(buffer.clone())
        });
    };
}

impl UserData for DrawCommandBuffer {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        draw_method!(methods, "arc", |(sx, sy, ex, ey, sd, ed)|: (f64, f64, f64, f64, f64, f64), {
            DrawCommand::Arc {
                sx, sy, ex, ey, sd, ed
            }
        });

        draw_method!(methods, "circle", |(ox, oy, px, py)|: (f64, f64, f64, f64), {
            DrawCommand::Circle {
                ox, oy, px, py
            }
        });

        draw_method!(methods, "color", |(x, y, paint_method)|: (f64, f64, PaintMethod), {
            DrawCommand::Color {
                x, y, paint_method
            }
        });

        draw_method!(methods, "ellipse", |(ox, oy, rx, ry, start, end)|: (f64, f64, f64, f64, f64, f64), {
            DrawCommand::Ellipse {
                ox, oy, rx, ry, start, end
            }
        });

        draw_method!(methods, "set_fill_color", |color|: String, {
            DrawCommand::SetFillColor {
                color
            }
        });

        draw_method!(methods, "set_fill_opacity", |opacity|: f64, {
            DrawCommand::SetFillOpacity(opacity)
        });

        draw_method!(methods, "set_fill_rule", |rule|: FillRule, {
            DrawCommand::SetFillRule(rule)
        });

        draw_method!(methods, "set_font", |font|: String, {
            DrawCommand::SetFont(font)
        });

        draw_method!(methods, "set_font_family", |font_family|: String, {
            DrawCommand::SetFontFamily(font_family)
        });

        draw_method!(methods, "set_font_size", |size|: f64, {
            DrawCommand::SetFontSize(size)
        });

        draw_method!(methods, "set_font_style", |style|: StyleType, {
            DrawCommand::SetFontStyle(style)
        });

        draw_method!(methods, "line", |(sx, sy, ex, ey)|: (f64, f64, f64, f64), {
            DrawCommand::Line { sx, sy, ex, ey }
        });

        draw_method!(methods, "matte", |(x, y, paint_method)|: (f64, f64, PaintMethod), {
            DrawCommand::Matte { x, y, paint_method }
        });

        draw_method!(methods, "path", |path|: PathCommandBuffer, {
            DrawCommand::Path(path.commands.lock().unwrap().clone())
        });

        draw_method!(methods, "rectangle", |(x1, y1, x2, y2)|: (f64, f64, f64, f64), {
            DrawCommand::Rectangle {
                x1, y1, x2, y2
            }
        });

        draw_method!(methods, "rotate", |degrees|: f64, {
            DrawCommand::Rotate(degrees)
        });

        draw_method!(methods, "round_rectangle", |(x1, y1, x2, y2, rx, ry)|: (f64, f64, f64, f64, f64, f64), {
            DrawCommand::RoundRectangle {
                x1, y1, x2, y2, rx, ry
            }
        });

        draw_method!(methods, "scale", |(x, y)|: (f64, f64), {
            DrawCommand::Scale {
                x, y
            }
        });

        draw_method!(methods, "skew_x", |degrees|: f64, {
            DrawCommand::SkewX(degrees)
        });

        draw_method!(methods, "skew_y", |degrees|: f64, {
            DrawCommand::SkewY(degrees)
        });

        draw_method!(methods, "set_stroke_antialias", |antialias|: u32, {
            DrawCommand::SetStrokeAntiAlias(antialias)
        });

        draw_method!(methods, "set_stroke_color", |color|: String, {
            DrawCommand::SetStrokeColor(color)
        });

        draw_method!(methods, "set_stroke_line_cap", |line_cap|: LineCap, {
            DrawCommand::SetStrokeLineCap(line_cap)
        });

        draw_method!(methods, "set_stroke_line_join", |line_join|: LineJoin, {
            DrawCommand::SetStrokeLineJoin(line_join)
        });

        draw_method!(methods, "set_stroke_width", |width|: f64, {
            DrawCommand::SetStrokeWidth(width)
        });

        draw_method!(methods, "set_text_antialias", |antialias|: u32, {
            DrawCommand::SetTextAntiAlias(antialias)
        });

        draw_method!(methods, "set_text_decoration", |decoration|: DecorationType, {
            DrawCommand::SetTextDecoration(decoration)
        });

        draw_method!(methods, "set_text_under_color", |color|: String, {
            DrawCommand::SetTextUnderColor(color)
        });

        draw_method!(methods, "text", |(align, x, y, text)|: (TextAlign, f64, f64, String), {
            DrawCommand::Text {
                align, x, y, text
            }
        });

        draw_method!(methods, "translate", |(x, y)|: (f64, f64), {
            DrawCommand::Translate {
                x, y
            }
        });

        draw_method!(methods, "set_viewbox", |(x1, y1, x2, y2)|: (u64, u64, u64, u64), {
            DrawCommand::SetViewbox {
                x1, y1, x2, y2
            }
        });

        methods.add_meta_method(MetaMethod::ToString, |state, buffer, (): ()| {
            state.create_string(&format!(
                "DrawCommandBuffer {{ commands = {} }}",
                buffer.commands.lock().unwrap().len()
            ))
        });
    }
}

enum DrawCommand {
    Arc {
        sx: f64,
        sy: f64,
        ex: f64,
        ey: f64,
        sd: f64,
        ed: f64,
    },
    Circle {
        ox: f64,
        oy: f64,
        px: f64,
        py: f64,
    },
    Color {
        x: f64,
        y: f64,
        paint_method: PaintMethod,
    },
    Ellipse {
        ox: f64,
        oy: f64,
        rx: f64,
        ry: f64,
        start: f64,
        end: f64,
    },
    SetFillColor {
        color: String,
    },
    SetFillOpacity(f64),
    SetFillRule(FillRule),
    SetFont(String),
    SetFontFamily(String),
    SetFontSize(f64),
    SetFontStyle(StyleType),
    Line {
        sx: f64,
        sy: f64,
        ex: f64,
        ey: f64,
    },
    Matte {
        x: f64,
        y: f64,
        paint_method: PaintMethod,
    },
    Path(Vec<PathCommand>),
    Rectangle {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    Rotate(f64),
    RoundRectangle {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        rx: f64,
        ry: f64,
    },
    Scale {
        x: f64,
        y: f64,
    },
    SkewX(f64),
    SkewY(f64),
    SetStrokeAntiAlias(u32),
    SetStrokeColor(String),
    SetStrokeLineCap(LineCap),
    SetStrokeLineJoin(LineJoin),
    SetStrokeWidth(f64),
    SetTextAntiAlias(u32),
    SetTextDecoration(DecorationType),
    SetTextUnderColor(String),
    Text {
        align: TextAlign,
        x: f64,
        y: f64,
        text: String,
    },
    Translate {
        x: f64,
        y: f64,
    },
    SetViewbox {
        x1: u64,
        y1: u64,
        x2: u64,
        y2: u64,
    },
}

#[derive(Clone, Default)]
struct PathCommandBuffer {
    commands: Arc<Mutex<Vec<PathCommand>>>,
}

macro_rules! path_method {
    ($methods:expr, $name:expr, |$args:pat|: $args_ty:ty, $block:block) => {
        $methods.add_method($name, |_state, path, $args: $args_ty| {
            let mut commands = path.commands.lock().unwrap();

            if commands.len() >= 512 {
                return Err(LuaError::RuntimeError(
                    "cannot have more than 512 path commands in one path".into(),
                ));
            }

            commands.push($block);
            Ok(path.clone())
        });
    };
}

impl UserData for PathCommandBuffer {
    fn add_methods<'a, M: UserDataMethods<'a, Self>>(methods: &mut M) {
        path_method!(methods, "close", |()|: (), {
            PathCommand::Close
        });

        path_method!(methods, "curve", |(x1, y1, x2, y2, x, y)|: (f64, f64, f64, f64, f64, f64), {
            PathCommand::Absolute {
                x1, y1, x2, y2, x, y
            }
        });

        path_method!(methods, "curve_relative", |(x1, y1, x2, y2, x, y)|: (f64, f64, f64, f64, f64, f64), {
            PathCommand::Relative {
                x1, y1, x2, y2, x, y
            }
        });

        path_method!(methods, "curve_quadratic_bezier", |(x1, y1, x, y)|: (f64, f64, f64, f64), {
            PathCommand::QuadraticBezierAbsolute {
                x1, y1, x, y
            }
        });

        path_method!(methods, "curve_quadratic_bezier_relative", |(x1, y1, x, y)|: (f64, f64, f64, f64), {
            PathCommand::QuadraticBezierRelative {
                x1, y1, x, y
            }
        });

        path_method!(methods, "curve_quadratic_bezier_smooth", |(x, y)|: (f64, f64), {
            PathCommand::QuadraticBezierSmoothAbsolute {
                x, y
            }
        });

        path_method!(methods, "curve_quadratic_bezier_smooth_relative", |(x, y)|: (f64, f64), {
            PathCommand::QuadraticBezierSmoothRelative {
                x, y
            }
        });

        path_method!(methods, "curve_smooth", |(x2, y2, x, y)|: (f64, f64, f64, f64), {
            PathCommand::SmoothAbsolute {
                x2, y2, x, y
            }
        });

        path_method!(methods, "curve_smooth_relative", |(x2, y2, x, y)|: (f64, f64, f64, f64), {
            PathCommand::SmoothRelative {
                x2, y2, x, y
            }
        });

        path_method!(methods, "elliptic_arc", |(rx, ry, x_axis_rotation, large_arc_flag, sweep_flag , x, y)|: (f64, f64, f64, u32, u32, f64, f64), {
            PathCommand::EllipticArcAbsolute {
                rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, x, y
            }
        });

        path_method!(methods, "elliptic_arc_relative", |(rx, ry, x_axis_rotation, large_arc_flag, sweep_flag , x, y)|: (f64, f64, f64, u32, u32, f64, f64), {
            PathCommand::EllipticArcRelative {
                rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, x, y
            }
        });

        path_method!(methods, "line", |(x, y)|: (f64, f64), {
            PathCommand::LineAbsolute {
                x, y
            }
        });

        path_method!(methods, "line_relative", |(x, y)|: (f64, f64), {
            PathCommand::LineRelative {
                x, y
            }
        });

        path_method!(methods, "line_horizontal", |x|: f64, {
            PathCommand::LineHorizontalAbsolute(x)
        });

        path_method!(methods, "line_horizontal_relative", |x|: f64, {
            PathCommand::LineHorizontalRelative(x)
        });

        path_method!(methods, "line_vertical", |y|: f64, {
            PathCommand::LineVerticalAbsolute(y)
        });

        path_method!(methods, "line_vertical_relative", |y|: f64, {
            PathCommand::LineVerticalRelative(y)
        });

        path_method!(methods, "move", |(x, y)|: (f64, f64), {
            PathCommand::MoveToAbsolute {
                x, y
            }
        });

        path_method!(methods, "move_relative", |(x, y)|: (f64, f64), {
            PathCommand::MoveToRelative {
                x, y
            }
        });

        methods.add_meta_method(MetaMethod::ToString, |state, path, (): ()| {
            state.create_string(&format!(
                "PathCommandBuffer {{ commands = {} }}",
                path.commands.lock().unwrap().len()
            ))
        });
    }
}

#[derive(Clone)]
enum PathCommand {
    Close,
    Absolute {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
    },
    Relative {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
    },
    QuadraticBezierAbsolute {
        x1: f64,
        y1: f64,
        x: f64,
        y: f64,
    },
    QuadraticBezierRelative {
        x1: f64,
        y1: f64,
        x: f64,
        y: f64,
    },
    QuadraticBezierSmoothAbsolute {
        x: f64,
        y: f64,
    },
    QuadraticBezierSmoothRelative {
        x: f64,
        y: f64,
    },
    SmoothAbsolute {
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
    },
    SmoothRelative {
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
    },
    EllipticArcAbsolute {
        rx: f64,
        ry: f64,
        x_axis_rotation: f64,
        large_arc_flag: u32,
        sweep_flag: u32,
        x: f64,
        y: f64,
    },
    EllipticArcRelative {
        rx: f64,
        ry: f64,
        x_axis_rotation: f64,
        large_arc_flag: u32,
        sweep_flag: u32,
        x: f64,
        y: f64,
    },
    LineAbsolute {
        x: f64,
        y: f64,
    },
    LineRelative {
        x: f64,
        y: f64,
    },
    LineHorizontalAbsolute(f64),
    LineHorizontalRelative(f64),
    LineVerticalAbsolute(f64),
    LineVerticalRelative(f64),
    MoveToAbsolute {
        x: f64,
        y: f64,
    },
    MoveToRelative {
        x: f64,
        y: f64,
    },
}
