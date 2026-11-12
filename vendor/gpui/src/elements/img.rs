use crate::{
    AnyElement, AnyImageCache, App, Asset, AssetLogger, Bounds, DefiniteLength, Element, ElementId,
    Entity, GlobalElementId, Hitbox, Image, ImageCache, InspectorElementId, InteractiveElement,
    ImageId, Interactivity, IntoElement, LayoutId, Length, ObjectFit, Pixels, RenderImage, Resource,
    SMOOTH_SVG_SCALE_FACTOR, SharedString, SharedUri, StyleRefinement, Styled, SvgRenderer,
    SvgSize, Task,
    Window, px, swap_rgba_pa_to_bgra,
};
use anyhow::{Context as _, Result};

use futures::{AsyncReadExt, Future};
use image::{
    AnimationDecoder, DynamicImage, Frame, ImageBuffer, ImageError, ImageFormat, Rgba,
    codecs::{gif::GifDecoder, webp::WebPDecoder},
};
use smallvec::SmallVec;
use std::{
    fs,
    io::{self, Cursor},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use util::ResultExt;

use super::{Stateful, StatefulInteractiveElement};

/// The delay before showing the loading state.
pub const LOADING_DELAY: Duration = Duration::from_millis(200);

/// A type alias to the resource loader that the `img()` element uses.
///
/// Note: that this is only for Resources, like URLs or file paths.
/// Custom loaders, or external images will not use this asset loader
pub type ImgResourceLoader = AssetLogger<ImageAssetLoader>;

/// A source of image content.
#[derive(Clone)]
pub enum ImageSource {
    /// The image content will be loaded from some resource location
    Resource(Resource),
    /// A resource decoded with SVG base URI and CSS options.
    ResourceWithSvgOptions {
        /// The resource to load.
        resource: Resource,
        /// The filename used to resolve relative SVG references.
        base_uri: Option<PathBuf>,
        /// CSS to apply while parsing the SVG.
        css: Option<String>,
    },
    /// Cached image data
    Render(Arc<RenderImage>),
    /// Cached image data
    Image(Arc<Image>),
    /// Cached image data with an SVG base URI for relative embedded resources.
    ImageWithBaseUri {
        /// The image data to decode.
        image: Arc<Image>,
        /// The filename used to resolve relative SVG references.
        base_uri: PathBuf,
    },
    /// Cached image data with SVG base URI and CSS options.
    ImageWithSvgOptions {
        /// The image data to decode.
        image: Arc<Image>,
        /// The filename used to resolve relative SVG references.
        base_uri: Option<PathBuf>,
        /// CSS to apply while parsing the SVG.
        css: Option<String>,
    },
    /// A custom loading function to use
    Custom(Arc<dyn Fn(&mut Window, &mut App) -> Option<Result<Arc<RenderImage>, ImageCacheError>>>),
}

fn is_uri(uri: &str) -> bool {
    http_client::Uri::from_str(uri).is_ok()
}

impl From<SharedUri> for ImageSource {
    fn from(value: SharedUri) -> Self {
        Self::Resource(Resource::Uri(value))
    }
}

impl<'a> From<&'a str> for ImageSource {
    fn from(s: &'a str) -> Self {
        if is_uri(s) {
            Self::Resource(Resource::Uri(s.to_string().into()))
        } else {
            Self::Resource(Resource::Embedded(s.to_string().into()))
        }
    }
}

impl From<String> for ImageSource {
    fn from(s: String) -> Self {
        if is_uri(&s) {
            Self::Resource(Resource::Uri(s.into()))
        } else {
            Self::Resource(Resource::Embedded(s.into()))
        }
    }
}

impl From<SharedString> for ImageSource {
    fn from(s: SharedString) -> Self {
        s.as_ref().into()
    }
}

impl From<&Path> for ImageSource {
    fn from(value: &Path) -> Self {
        Self::Resource(value.to_path_buf().into())
    }
}

impl From<Arc<Path>> for ImageSource {
    fn from(value: Arc<Path>) -> Self {
        Self::Resource(value.into())
    }
}

impl From<PathBuf> for ImageSource {
    fn from(value: PathBuf) -> Self {
        Self::Resource(value.into())
    }
}

impl From<Arc<RenderImage>> for ImageSource {
    fn from(value: Arc<RenderImage>) -> Self {
        Self::Render(value)
    }
}

impl From<Arc<Image>> for ImageSource {
    fn from(value: Arc<Image>) -> Self {
        Self::Image(value)
    }
}

impl<F> From<F> for ImageSource
where
    F: Fn(&mut Window, &mut App) -> Option<Result<Arc<RenderImage>, ImageCacheError>> + 'static,
{
    fn from(value: F) -> Self {
        Self::Custom(Arc::new(value))
    }
}

/// The style of an image element.
pub struct ImageStyle {
    grayscale: bool,
    transform_smoothing: Option<bool>,
    object_fit: ObjectFit,
    loading: Option<Box<dyn Fn() -> AnyElement>>,
    fallback: Option<Box<dyn Fn() -> AnyElement>>,
}

/// Limit the source image size before it is fully decoded.
#[derive(Clone, Copy, Debug)]
pub enum ImageSizeLimit {
    /// Maximum width and height in source pixels.
    Pixels(u64),
    /// Maximum fraction of the current window's pixel dimensions.
    FrameRatio(f64),
}

impl ImageSizeLimit {
    fn resolve(self, window: &Window) -> (u64, u64) {
        match self {
            Self::Pixels(limit) => (limit, limit),
            Self::FrameRatio(ratio) => {
                let ratio = if ratio.is_finite() {
                    ratio.max(0.0)
                } else if ratio.is_sign_positive() {
                    f64::MAX
                } else {
                    0.0
                };
                let scale = f64::from(window.scale_factor());
                let viewport = window.viewport_size();
                let width = (f64::from(viewport.width.0) * scale * ratio)
                    .floor()
                    .clamp(0.0, u64::MAX as f64) as u64;
                let height = (f64::from(viewport.height.0) * scale * ratio)
                    .floor()
                    .clamp(0.0, u64::MAX as f64) as u64;
                (width, height)
            }
        }
    }
}

impl Default for ImageStyle {
    fn default() -> Self {
        Self {
            grayscale: false,
            transform_smoothing: None,
            object_fit: ObjectFit::Contain,
            loading: None,
            fallback: None,
        }
    }
}

/// Style an image element.
pub trait StyledImage: Sized {
    /// Get a mutable [ImageStyle] from the element.
    fn image_style(&mut self) -> &mut ImageStyle;

    /// Set the image to be displayed in grayscale.
    fn grayscale(mut self, grayscale: bool) -> Self {
        self.image_style().grayscale = grayscale;
        self
    }

    /// Set the object fit for the image.
    fn object_fit(mut self, object_fit: ObjectFit) -> Self {
        self.image_style().object_fit = object_fit;
        self
    }

    /// Set the object fit for the image.
    fn with_fallback(mut self, fallback: impl Fn() -> AnyElement + 'static) -> Self {
        self.image_style().fallback = Some(Box::new(fallback));
        self
    }

    /// Set the object fit for the image.
    fn with_loading(mut self, loading: impl Fn() -> AnyElement + 'static) -> Self {
        self.image_style().loading = Some(Box::new(loading));
        self
    }
}

impl StyledImage for Img {
    fn image_style(&mut self) -> &mut ImageStyle {
        &mut self.style
    }
}

impl StyledImage for Stateful<Img> {
    fn image_style(&mut self) -> &mut ImageStyle {
        &mut self.element.style
    }
}

/// An image element.
pub struct Img {
    interactivity: Interactivity,
    source: ImageSource,
    style: ImageStyle,
    image_cache: Option<AnyImageCache>,
    on_render_image_loaded: Option<Box<dyn Fn(&Arc<RenderImage>) + 'static>>,
    /// Fixed frame for multi-frame images; `None` preserves animation.
    frame_index: Option<usize>,
    disabled_conversion: bool,
    edge_detection: Option<([i32; 9], i32)>,
    max_image_size: Option<ImageSizeLimit>,
}

/// Create a new image element.
#[track_caller]
pub fn img(source: impl Into<ImageSource>) -> Img {
    Img {
        interactivity: Interactivity::new(),
        source: source.into(),
        style: ImageStyle::default(),
        image_cache: None,
        on_render_image_loaded: None,
        frame_index: None,
        disabled_conversion: false,
        edge_detection: None,
        max_image_size: None,
    }
}

impl Img {
    fn image_conversion(&self) -> Option<ImageConversion> {
        if self.disabled_conversion {
            Some(ImageConversion::Disabled)
        } else {
            self.edge_detection
                .map(|(matrix, color_adjust)| ImageConversion::EdgeDetection {
                    matrix,
                    color_adjust,
                })
        }
    }

    fn convert_image(
        image: Arc<RenderImage>,
        conversion: Option<ImageConversion>,
        state: &mut Option<ImgState>,
    ) -> Arc<RenderImage> {
        let Some(conversion) = conversion else {
            if let Some(state) = state {
                state.converted_image = None;
            }
            return image;
        };

        if let Some((image_id, cached_conversion, converted)) = state
            .as_ref()
            .and_then(|state| state.converted_image.as_ref())
        {
            if *image_id == image.id && *cached_conversion == conversion {
                return Arc::clone(converted);
            }
        }

        let converted = Arc::new(match conversion {
            ImageConversion::Disabled => image.disabled_conversion(),
            ImageConversion::EdgeDetection {
                matrix,
                color_adjust,
            } => image.edge_detected(matrix, color_adjust),
        });
        if let Some(state) = state {
            state.converted_image = Some((image.id, conversion, Arc::clone(&converted)));
        }
        converted
    }

    /// A list of all format extensions currently supported by this img element
    pub fn extensions() -> &'static [&'static str] {
        // This is the list in [image::ImageFormat::from_extension] + `svg`
        &[
            "avif", "jpg", "jpeg", "png", "gif", "webp", "tif", "tiff", "tga", "dds", "bmp", "ico",
            "hdr", "exr", "pbm", "pam", "ppm", "pgm", "ff", "farbfeld", "qoi", "svg",
        ]
    }

    /// Sets the image cache for the current node.
    ///
    /// If the `image_cache` is not explicitly provided, the function will determine the image cache by:
    ///
    /// 1. Checking if any ancestor node of the current node contains an `ImageCacheElement`, If such a node exists, the image cache specified by that ancestor will be used.
    /// 2. If no ancestor node contains an `ImageCacheElement`, the global image cache will be used as a fallback.
    ///
    /// This mechanism provides a flexible way to manage image caching, allowing precise control when needed,
    /// while ensuring a default behavior when no cache is explicitly specified.
    #[inline]
    pub fn image_cache<I: ImageCache>(self, image_cache: &Entity<I>) -> Self {
        Self {
            image_cache: Some(image_cache.clone().into()),
            ..self
        }
    }

    /// Observe the decoded image data when it is available to this element.
    pub fn on_render_image_loaded(
        mut self,
        callback: impl Fn(&Arc<RenderImage>) + 'static,
    ) -> Self {
        self.on_render_image_loaded = Some(Box::new(callback));
        self
    }

    /// Display one frame of a multi-frame image instead of animating it.
    pub fn frame_index(mut self, frame_index: usize) -> Self {
        self.frame_index = Some(frame_index);
        self
    }

    /// Apply GNU Emacs' disabled-image conversion before displaying the image.
    pub fn disabled_conversion(mut self) -> Self {
        self.disabled_conversion = true;
        self
    }

    /// Apply a 3×3 edge-detection matrix before displaying the image.
    pub fn edge_detection(mut self, matrix: [i32; 9], color_adjust: i32) -> Self {
        self.edge_detection = Some((matrix, color_adjust));
        self
    }

    /// Set whether transformed pixels are smoothed with bilinear sampling.
    /// `false` uses nearest-neighbor sampling; `true` uses linear sampling.
    pub fn transform_smoothing(mut self, smoothing: bool) -> Self {
        self.style.transform_smoothing = Some(smoothing);
        self
    }

    /// Refuse to decode images whose source dimensions exceed `limit`.
    pub fn max_image_size(mut self, limit: ImageSizeLimit) -> Self {
        self.max_image_size = Some(limit);
        self
    }
}

impl Deref for Stateful<Img> {
    type Target = Img;

    fn deref(&self) -> &Self::Target {
        &self.element
    }
}

impl DerefMut for Stateful<Img> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.element
    }
}

/// The image state between frames
struct ImgState {
    frame_index: usize,
    last_frame_time: Option<Instant>,
    started_loading: Option<(Instant, Task<()>)>,
    converted_image: Option<(ImageId, ImageConversion, Arc<RenderImage>)>,
    loaded_image: Option<(ImageDimensionsSource, Arc<RenderImage>)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImageConversion {
    Disabled,
    EdgeDetection {
        matrix: [i32; 9],
        color_adjust: i32,
    },
}

/// The image layout state between frames
pub struct ImgLayoutState {
    frame_index: usize,
    replacement: Option<AnyElement>,
    image_data: Option<Arc<RenderImage>>,
    max_image_size: Option<(u64, u64)>,
}

impl Element for Img {
    type RequestLayoutState = ImgLayoutState;
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        self.interactivity.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        self.interactivity.source_location()
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut layout_state = ImgLayoutState {
            frame_index: 0,
            replacement: None,
            image_data: None,
            max_image_size: None,
        };

        window.with_optional_element_state(global_id, |state, window| {
            let max_image_size = self.max_image_size.map(|limit| limit.resolve(window));
            layout_state.max_image_size = max_image_size;
            let mut state = state.map(|state| {
                state.unwrap_or(ImgState {
                    frame_index: 0,
                    last_frame_time: None,
                    started_loading: None,
                    converted_image: None,
                    loaded_image: None,
                })
            });

            let mut frame_index = self
                .frame_index
                .unwrap_or_else(|| state.as_ref().map(|state| state.frame_index).unwrap_or(0));
            let conversion = self.image_conversion();
            let source_key = self.source.image_dimensions_source();
            if let Some(state) = &mut state
                && state
                    .loaded_image
                    .as_ref()
                    .is_some_and(|(key, _)| Some(key) != source_key.as_ref())
            {
                state.loaded_image = None;
                state.converted_image = None;
            }
            let mut cached_image = state.as_ref().and_then(|state| {
                state
                    .loaded_image
                    .as_ref()
                    .filter(|(key, _)| Some(key) == source_key.as_ref())
                    .map(|(_, image)| Arc::clone(image))
            });

            let layout_id = self.interactivity.request_layout(
                global_id,
                inspector_id,
                window,
                cx,
                |mut style, window, cx| {
                    let mut replacement_id = None;

                    let loaded_data = match cached_image.take() {
                        Some(image) => Some(Ok(image)),
                        None => self.source.use_data_with_size_limit(
                            self.image_cache
                                .clone()
                                .or_else(|| window.image_cache_stack.last().cloned()),
                            max_image_size,
                            window,
                            cx,
                        ),
                    };
                    match loaded_data {
                        Some(Ok(data)) => {
                            if let Some(on_loaded) = &self.on_render_image_loaded {
                                on_loaded(&data);
                            }
                            if let (Some(key), Some(state)) = (source_key.as_ref(), state.as_mut()) {
                                state.loaded_image = Some((key.clone(), Arc::clone(&data)));
                            }
                            let data = Self::convert_image(data, conversion, &mut state);
                            layout_state.image_data = Some(Arc::clone(&data));
                            let frame_count = data.frame_count().max(1);
                            frame_index = self
                                .frame_index
                                .map(|index| index.min(frame_count - 1))
                                .unwrap_or_else(|| frame_index.min(frame_count - 1));
                            if let Some(state) = &mut state {
                                if self.frame_index.is_none() && frame_count > 1 {
                                    state.frame_index = state.frame_index.min(frame_count - 1);
                                    let current_time = Instant::now();
                                    if let Some(last_frame_time) = state.last_frame_time {
                                        let elapsed = current_time - last_frame_time;
                                        let frame_duration =
                                            Duration::from(data.delay(state.frame_index));

                                        if elapsed >= frame_duration {
                                            state.frame_index =
                                                (state.frame_index + 1) % frame_count;
                                            state.last_frame_time =
                                                Some(current_time - (elapsed - frame_duration));
                                        }
                                    } else {
                                        state.last_frame_time = Some(current_time);
                                    }
                                }
                                state.started_loading = None;
                            }

                            let image_size = data.render_size(frame_index);
                            style.aspect_ratio = Some(image_size.width / image_size.height);

                            if let Length::Auto = style.size.width {
                                style.size.width = match style.size.height {
                                    Length::Definite(DefiniteLength::Absolute(abs_length)) => {
                                        let height_px = abs_length.to_pixels(window.rem_size());
                                        Length::Definite(
                                            px(image_size.width.0 * height_px.0
                                                / image_size.height.0)
                                            .into(),
                                        )
                                    }
                                    _ => Length::Definite(image_size.width.into()),
                                };
                            }

                            if let Length::Auto = style.size.height {
                                style.size.height = match style.size.width {
                                    Length::Definite(DefiniteLength::Absolute(abs_length)) => {
                                        let width_px = abs_length.to_pixels(window.rem_size());
                                        Length::Definite(
                                            px(image_size.height.0 * width_px.0
                                                / image_size.width.0)
                                            .into(),
                                        )
                                    }
                                    _ => Length::Definite(image_size.height.into()),
                                };
                            }

                            if global_id.is_some()
                                && self.frame_index.is_none()
                                && frame_count > 1
                            {
                                window.request_animation_frame();
                            }
                        }
                        Some(_err) => {
                            if let Some(fallback) = self.style.fallback.as_ref() {
                                let mut element = fallback();
                                replacement_id = Some(element.request_layout(window, cx));
                                layout_state.replacement = Some(element);
                            }
                            if let Some(state) = &mut state {
                                state.started_loading = None;
                            }
                        }
                        None => {
                            if let Some(state) = &mut state {
                                if let Some((started_loading, _)) = state.started_loading {
                                    if started_loading.elapsed() > LOADING_DELAY
                                        && let Some(loading) = self.style.loading.as_ref()
                                    {
                                        let mut element = loading();
                                        replacement_id = Some(element.request_layout(window, cx));
                                        layout_state.replacement = Some(element);
                                    }
                                } else {
                                    let current_view = window.current_view();
                                    let task = window.spawn(cx, async move |cx| {
                                        cx.background_executor().timer(LOADING_DELAY).await;
                                        cx.update(move |_, cx| {
                                            cx.notify(current_view);
                                        })
                                        .ok();
                                    });
                                    state.started_loading = Some((Instant::now(), task));
                                }
                            }
                        }
                    }

                    window.request_layout(style, replacement_id, cx)
                },
            );

            layout_state.frame_index = frame_index;

            ((layout_id, layout_state), state)
        })
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            bounds.size,
            window,
            cx,
            |_, _, hitbox, window, cx| {
                if let Some(replacement) = &mut request_layout.replacement {
                    replacement.prepaint(window, cx);
                }

                hitbox
            },
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_state: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let source = self.source.clone();
        let conversion = self.image_conversion();
        self.interactivity.paint(
            global_id,
            inspector_id,
            bounds,
            hitbox.as_ref(),
            window,
            cx,
            |style, window, cx| {
                let image_data = match layout_state.image_data.clone() {
                    Some(data) => Some(Ok(data)),
                    None => source
                        .use_data_with_size_limit(
                            self.image_cache
                                .clone()
                                .or_else(|| window.image_cache_stack.last().cloned()),
                            layout_state.max_image_size,
                            window,
                            cx,
                        )
                        .map(|result| {
                            result.map(|data| {
                                let mut state = None;
                                Self::convert_image(data, conversion, &mut state)
                            })
                        }),
                };
                if let Some(Ok(data)) = image_data {
                    let new_bounds = self
                        .style
                        .object_fit
                        .get_bounds(bounds, data.size(layout_state.frame_index));
                    let corner_radii = style
                        .corner_radii
                        .to_pixels(window.rem_size())
                        .clamp_radii_for_quad_size(new_bounds.size);
                    window
                        .paint_image(
                            new_bounds,
                            corner_radii,
                            data,
                            layout_state.frame_index,
                            self.style.grayscale,
                            self.style.transform_smoothing,
                        )
                        .log_err();
                } else if let Some(replacement) = &mut layout_state.replacement {
                    replacement.paint(window, cx);
                }
            },
        )
    }
}

impl Styled for Img {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.interactivity.base_style
    }
}

impl InteractiveElement for Img {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

impl IntoElement for Img {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl StatefulInteractiveElement for Img {}

#[derive(Clone, Hash, PartialEq, Eq)]
enum ImageDimensionsSource {
    Resource {
        resource: Resource,
        base_uri: Option<PathBuf>,
        css: Option<String>,
    },
    Image {
        image: Arc<Image>,
        base_uri: Option<PathBuf>,
        css: Option<String>,
    },
}

#[derive(Clone)]
enum ImageDimensionsLoader {}

impl Asset for ImageDimensionsLoader {
    type Source = ImageDimensionsSource;
    type Output = Result<(u32, u32), ImageCacheError>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let client = cx.http_client();
        let svg_renderer = cx.svg_renderer();
        let asset_source = cx.asset_source().clone();
        async move {
            if let ImageDimensionsSource::Resource {
                resource: Resource::Path(path),
                ..
            } = &source
            {
                let reader = image::ImageReader::open(path.as_ref())?.with_guessed_format()?;
                if reader.format().is_some() {
                    return Ok(reader.into_dimensions()?);
                }
            }

            let (bytes, format_hint, base_uri, css) = match source {
                ImageDimensionsSource::Resource {
                    resource,
                    base_uri,
                    css,
                } => {
                    let bytes = match resource.clone() {
                        Resource::Path(path) => fs::read(path.as_ref())?,
                        Resource::Uri(uri) => {
                            let mut response = client
                                .get(uri.as_ref(), ().into(), true)
                                .await
                                .with_context(|| format!("loading image asset from {uri:?}"))?;
                            let mut body = Vec::new();
                            response.body_mut().read_to_end(&mut body).await?;
                            if !response.status().is_success() {
                                let mut body = String::from_utf8_lossy(&body).into_owned();
                                let first_line = body.lines().next().unwrap_or("").trim_end();
                                body.truncate(first_line.len());
                                return Err(ImageCacheError::BadStatus {
                                    uri,
                                    status: response.status(),
                                    body,
                                });
                            }
                            body
                        }
                        Resource::Embedded(path) => {
                            let data = asset_source.load(&path).ok().flatten();
                            if let Some(data) = data {
                                data.to_vec()
                            } else {
                                return Err(ImageCacheError::Asset(
                                    format!("Embedded resource not found: {}", path).into(),
                                ));
                            }
                        }
                    };
                    (bytes, None, base_uri, css)
                }
                ImageDimensionsSource::Image {
                    image,
                    base_uri,
                    css,
                } => (image.bytes.clone(), Some(image.format), base_uri, css),
            };

            image_dimensions_from_bytes(
                &bytes,
                format_hint,
                &svg_renderer,
                base_uri.as_deref(),
                css.as_deref(),
            )
        }
    }
}

fn image_decoder_format(format: crate::ImageFormat) -> Option<image::ImageFormat> {
    match format {
        crate::ImageFormat::Png => Some(image::ImageFormat::Png),
        crate::ImageFormat::Jpeg => Some(image::ImageFormat::Jpeg),
        crate::ImageFormat::Webp => Some(image::ImageFormat::WebP),
        crate::ImageFormat::Gif => Some(image::ImageFormat::Gif),
        crate::ImageFormat::Svg => None,
        crate::ImageFormat::Bmp => Some(image::ImageFormat::Bmp),
        crate::ImageFormat::Tiff => Some(image::ImageFormat::Tiff),
    }
}

fn image_dimensions_from_bytes(
    bytes: &[u8],
    format_hint: Option<crate::ImageFormat>,
    svg_renderer: &SvgRenderer,
    base_uri: Option<&Path>,
    css: Option<&str>,
) -> Result<(u32, u32), ImageCacheError> {
    let is_svg = matches!(format_hint, Some(crate::ImageFormat::Svg))
        || (format_hint.is_none() && image::guess_format(bytes).is_err());
    if is_svg {
        return Ok(svg_renderer.intrinsic_dimensions_with_svg_options(bytes, base_uri, css)?);
    }

    let format = format_hint
        .and_then(image_decoder_format)
        .or_else(|| image::guess_format(bytes).ok())
        .context("unrecognized image format")?;
    Ok(image::ImageReader::with_format(Cursor::new(bytes), format).into_dimensions()?)
}

fn image_exceeds_limit(width: u32, height: u32, limit: (u64, u64)) -> bool {
    u64::from(width) > limit.0 || u64::from(height) > limit.1
}

impl ImageSource {
    pub(crate) fn use_data(
        &self,
        cache: Option<AnyImageCache>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        match self {
            ImageSource::Resource(resource) => {
                if let Some(cache) = cache {
                    cache.load(resource, window, cx)
                } else {
                    window.use_asset::<ImgResourceLoader>(resource, cx)
                }
            }
            ImageSource::ResourceWithSvgOptions {
                resource,
                base_uri,
                css,
            } => {
                let source = (resource.clone(), base_uri.clone(), css.clone());
                window.use_asset::<AssetLogger<ImageAssetLoaderWithSvgOptions>>(&source, cx)
            }
            ImageSource::Custom(loading_fn) => loading_fn(window, cx),
            ImageSource::Render(data) => Some(Ok(data.to_owned())),
            ImageSource::Image(data) => window.use_asset::<AssetLogger<ImageDecoder>>(data, cx),
            ImageSource::ImageWithBaseUri { image, base_uri } => {
                let source = (image.clone(), Some(base_uri.clone()), None);
                window.use_asset::<AssetLogger<ImageDecoderWithSvgOptions>>(&source, cx)
            }
            ImageSource::ImageWithSvgOptions {
                image,
                base_uri,
                css,
            } => {
                let source = (image.clone(), base_uri.clone(), css.clone());
                window.use_asset::<AssetLogger<ImageDecoderWithSvgOptions>>(&source, cx)
            }
        }
    }

    pub(crate) fn use_data_with_size_limit(
        &self,
        cache: Option<AnyImageCache>,
        limit: Option<(u64, u64)>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        let Some(limit) = limit else {
            return self.use_data(cache, window, cx);
        };

        if let Some(source) = self.image_dimensions_source() {
            return match window.use_asset::<AssetLogger<ImageDimensionsLoader>>(&source, cx) {
                None => None,
                Some(Err(error)) => Some(Err(error)),
                Some(Ok((width, height))) if image_exceeds_limit(width, height, limit) => {
                    Some(Err(ImageCacheError::ImageTooLarge {
                        width,
                        height,
                        max_width: limit.0,
                        max_height: limit.1,
                    }))
                }
                Some(Ok(_)) => self.use_data(cache, window, cx),
            };
        }

        self.use_data(cache, window, cx).map(|result| {
            result.and_then(|image| {
                if image.frame_count() == 0 {
                    return Ok(image);
                }
                let size = image.size(0);
                if image_exceeds_limit(size.width.0 as u32, size.height.0 as u32, limit) {
                    Err(ImageCacheError::ImageTooLarge {
                        width: size.width.0 as u32,
                        height: size.height.0 as u32,
                        max_width: limit.0,
                        max_height: limit.1,
                    })
                } else {
                    Ok(image)
                }
            })
        })
    }

    fn image_dimensions_source(&self) -> Option<ImageDimensionsSource> {
        match self {
            ImageSource::Resource(resource) => Some(ImageDimensionsSource::Resource {
                resource: resource.clone(),
                base_uri: None,
                css: None,
            }),
            ImageSource::ResourceWithSvgOptions {
                resource,
                base_uri,
                css,
            } => Some(ImageDimensionsSource::Resource {
                resource: resource.clone(),
                base_uri: base_uri.clone(),
                css: css.clone(),
            }),
            ImageSource::Image(image) => Some(ImageDimensionsSource::Image {
                image: image.clone(),
                base_uri: None,
                css: None,
            }),
            ImageSource::ImageWithBaseUri { image, base_uri } => {
                Some(ImageDimensionsSource::Image {
                    image: image.clone(),
                    base_uri: Some(base_uri.clone()),
                    css: None,
                })
            }
            ImageSource::ImageWithSvgOptions {
                image,
                base_uri,
                css,
            } => Some(ImageDimensionsSource::Image {
                image: image.clone(),
                base_uri: base_uri.clone(),
                css: css.clone(),
            }),
            ImageSource::Render(_) | ImageSource::Custom(_) => None,
        }
    }

    pub(crate) fn get_data(
        &self,
        cache: Option<AnyImageCache>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        match self {
            ImageSource::Resource(resource) => {
                if let Some(cache) = cache {
                    cache.load(resource, window, cx)
                } else {
                    window.get_asset::<ImgResourceLoader>(resource, cx)
                }
            }
            ImageSource::ResourceWithSvgOptions {
                resource,
                base_uri,
                css,
            } => {
                let source = (resource.clone(), base_uri.clone(), css.clone());
                window.get_asset::<AssetLogger<ImageAssetLoaderWithSvgOptions>>(&source, cx)
            }
            ImageSource::Custom(loading_fn) => loading_fn(window, cx),
            ImageSource::Render(data) => Some(Ok(data.to_owned())),
            ImageSource::Image(data) => window.get_asset::<AssetLogger<ImageDecoder>>(data, cx),
            ImageSource::ImageWithBaseUri { image, base_uri } => {
                let source = (image.clone(), Some(base_uri.clone()), None);
                window.get_asset::<AssetLogger<ImageDecoderWithSvgOptions>>(&source, cx)
            }
            ImageSource::ImageWithSvgOptions {
                image,
                base_uri,
                css,
            } => {
                let source = (image.clone(), base_uri.clone(), css.clone());
                window.get_asset::<AssetLogger<ImageDecoderWithSvgOptions>>(&source, cx)
            }
        }
    }

    /// Remove this image source from the asset system
    pub fn remove_asset(&self, cx: &mut App) {
        match self {
            ImageSource::Resource(resource) => {
                cx.remove_asset::<ImgResourceLoader>(resource);
            }
            ImageSource::ResourceWithSvgOptions {
                resource,
                base_uri,
                css,
            } => {
                let source = (resource.clone(), base_uri.clone(), css.clone());
                cx.remove_asset::<AssetLogger<ImageAssetLoaderWithSvgOptions>>(&source);
            }
            ImageSource::Custom(_) | ImageSource::Render(_) => {}
            ImageSource::Image(data) => cx.remove_asset::<AssetLogger<ImageDecoder>>(data),
            ImageSource::ImageWithBaseUri { image, base_uri } => {
                let source = (image.clone(), Some(base_uri.clone()), None);
                cx.remove_asset::<AssetLogger<ImageDecoderWithSvgOptions>>(&source);
            }
            ImageSource::ImageWithSvgOptions {
                image,
                base_uri,
                css,
            } => {
                let source = (image.clone(), base_uri.clone(), css.clone());
                cx.remove_asset::<AssetLogger<ImageDecoderWithSvgOptions>>(&source);
            }
        }
    }
}

#[derive(Clone)]
enum ImageDecoder {}

impl Asset for ImageDecoder {
    type Source = Arc<Image>;
    type Output = Result<Arc<RenderImage>, ImageCacheError>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let renderer = cx.svg_renderer();
        async move { source.to_image_data(renderer).map_err(Into::into) }
    }
}

#[derive(Clone)]
enum ImageDecoderWithSvgOptions {}

impl Asset for ImageDecoderWithSvgOptions {
    type Source = (Arc<Image>, Option<PathBuf>, Option<String>);
    type Output = Result<Arc<RenderImage>, ImageCacheError>;

    fn load(
        (image, base_uri, css): Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let renderer = cx.svg_renderer();
        async move {
            image
                .to_image_data_with_svg_options(renderer, base_uri.as_deref(), css.as_deref())
                .map_err(Into::into)
        }
    }
}

/// An image loader for the GPUI asset system
#[derive(Clone)]
pub enum ImageAssetLoader {}

impl Asset for ImageAssetLoader {
    type Source = Resource;
    type Output = Result<Arc<RenderImage>, ImageCacheError>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let client = cx.http_client();
        // TODO: Can we make SVGs always rescale?
        // let scale_factor = cx.scale_factor();
        let svg_renderer = cx.svg_renderer();
        let asset_source = cx.asset_source().clone();
        async move {
            let bytes = match source.clone() {
                Resource::Path(uri) => fs::read(uri.as_ref())?,
                Resource::Uri(uri) => {
                    let mut response = client
                        .get(uri.as_ref(), ().into(), true)
                        .await
                        .with_context(|| format!("loading image asset from {uri:?}"))?;
                    let mut body = Vec::new();
                    response.body_mut().read_to_end(&mut body).await?;
                    if !response.status().is_success() {
                        let mut body = String::from_utf8_lossy(&body).into_owned();
                        let first_line = body.lines().next().unwrap_or("").trim_end();
                        body.truncate(first_line.len());
                        return Err(ImageCacheError::BadStatus {
                            uri,
                            status: response.status(),
                            body,
                        });
                    }
                    body
                }
                Resource::Embedded(path) => {
                    let data = asset_source.load(&path).ok().flatten();
                    if let Some(data) = data {
                        data.to_vec()
                    } else {
                        return Err(ImageCacheError::Asset(
                            format!("Embedded resource not found: {}", path).into(),
                        ));
                    }
                }
            };

            let data = if let Ok(format) = image::guess_format(&bytes) {
                let data = match format {
                    ImageFormat::Gif => {
                        let decoder = GifDecoder::new(Cursor::new(&bytes))?;
                        let mut frames = SmallVec::new();

                        for frame in decoder.into_frames() {
                            let mut frame = frame?;
                            // Convert from RGBA to BGRA.
                            for pixel in frame.buffer_mut().chunks_exact_mut(4) {
                                pixel.swap(0, 2);
                            }
                            frames.push(frame);
                        }

                        frames
                    }
                    ImageFormat::WebP => {
                        let mut decoder = WebPDecoder::new(Cursor::new(&bytes))?;

                        if decoder.has_animation() {
                            let _ = decoder.set_background_color(Rgba([0, 0, 0, 0]));
                            let mut frames = SmallVec::new();

                            for frame in decoder.into_frames() {
                                let mut frame = frame?;
                                // Convert from RGBA to BGRA.
                                for pixel in frame.buffer_mut().chunks_exact_mut(4) {
                                    pixel.swap(0, 2);
                                }
                                frames.push(frame);
                            }

                            frames
                        } else {
                            let mut data = DynamicImage::from_decoder(decoder)?.into_rgba8();

                            // Convert from RGBA to BGRA.
                            for pixel in data.chunks_exact_mut(4) {
                                pixel.swap(0, 2);
                            }

                            SmallVec::from_elem(Frame::new(data), 1)
                        }
                    }
                    _ => {
                        let mut data =
                            image::load_from_memory_with_format(&bytes, format)?.into_rgba8();

                        // Convert from RGBA to BGRA.
                        for pixel in data.chunks_exact_mut(4) {
                            pixel.swap(0, 2);
                        }

                        SmallVec::from_elem(Frame::new(data), 1)
                    }
                };

                RenderImage::new(data)
            } else {
                let pixmap =
                    // TODO: Can we make svgs always rescale?
                    svg_renderer.render_pixmap(&bytes, SvgSize::ScaleFactor(SMOOTH_SVG_SCALE_FACTOR))?;

                let mut buffer =
                    ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take()).unwrap();

                for pixel in buffer.chunks_exact_mut(4) {
                    swap_rgba_pa_to_bgra(pixel);
                }

                let mut image = RenderImage::new(SmallVec::from_elem(Frame::new(buffer), 1));
                image.scale_factor = SMOOTH_SVG_SCALE_FACTOR;
                image
            };

            Ok(Arc::new(data))
        }
    }
}

/// An SVG resource loader that applies descriptor-specific URI and CSS options.
#[derive(Clone)]
pub enum ImageAssetLoaderWithSvgOptions {}

impl Asset for ImageAssetLoaderWithSvgOptions {
    type Source = (Resource, Option<PathBuf>, Option<String>);
    type Output = Result<Arc<RenderImage>, ImageCacheError>;

    fn load(
        (source, base_uri, css): Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let client = cx.http_client();
        let svg_renderer = cx.svg_renderer();
        let asset_source = cx.asset_source().clone();
        let base_uri = base_uri.or_else(|| match &source {
            Resource::Path(path) => Some(path.to_path_buf()),
            Resource::Uri(_) | Resource::Embedded(_) => None,
        });
        async move {
            let bytes = match source {
                Resource::Path(path) => fs::read(path.as_ref())?,
                Resource::Uri(uri) => {
                    let mut response = client
                        .get(uri.as_ref(), ().into(), true)
                        .await
                        .with_context(|| format!("loading image asset from {uri:?}"))?;
                    let mut body = Vec::new();
                    response.body_mut().read_to_end(&mut body).await?;
                    if !response.status().is_success() {
                        let mut body = String::from_utf8_lossy(&body).into_owned();
                        let first_line = body.lines().next().unwrap_or("").trim_end();
                        body.truncate(first_line.len());
                        return Err(ImageCacheError::BadStatus {
                            uri,
                            status: response.status(),
                            body,
                        });
                    }
                    body
                }
                Resource::Embedded(path) => {
                    let data = asset_source.load(&path).ok().flatten();
                    if let Some(data) = data {
                        data.to_vec()
                    } else {
                        return Err(ImageCacheError::Asset(
                            format!("Embedded resource not found: {}", path).into(),
                        ));
                    }
                }
            };

            let pixmap = svg_renderer.render_pixmap_with_svg_options(
                &bytes,
                SvgSize::ScaleFactor(SMOOTH_SVG_SCALE_FACTOR),
                base_uri.as_deref(),
                css.as_deref(),
            )?;
            let mut buffer =
                ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take()).unwrap();
            for pixel in buffer.chunks_exact_mut(4) {
                swap_rgba_pa_to_bgra(pixel);
            }

            let mut image = RenderImage::new(SmallVec::from_elem(Frame::new(buffer), 1));
            image.scale_factor = SMOOTH_SVG_SCALE_FACTOR;
            Ok(Arc::new(image))
        }
    }
}

/// An error that can occur when interacting with the image cache.
#[derive(Debug, Error, Clone)]
pub enum ImageCacheError {
    /// The image dimensions exceed the configured `max-image-size` value.
    #[error("image {width}x{height} exceeds max-image-size {max_width}x{max_height}")]
    ImageTooLarge {
        /// Actual image width in pixels.
        width: u32,
        /// Actual image height in pixels.
        height: u32,
        /// Maximum allowed width in pixels.
        max_width: u64,
        /// Maximum allowed height in pixels.
        max_height: u64,
    },
    /// Some other kind of error occurred
    #[error("error: {0}")]
    Other(#[from] Arc<anyhow::Error>),
    /// An error that occurred while reading the image from disk.
    #[error("IO error: {0}")]
    Io(Arc<std::io::Error>),
    /// An error that occurred while processing an image.
    #[error("unexpected http status for {uri}: {status}, body: {body}")]
    BadStatus {
        /// The URI of the image.
        uri: SharedUri,
        /// The HTTP status code.
        status: http_client::StatusCode,
        /// The HTTP response body.
        body: String,
    },
    /// An error that occurred while processing an asset.
    #[error("asset error: {0}")]
    Asset(SharedString),
    /// An error that occurred while processing an image.
    #[error("image error: {0}")]
    Image(Arc<ImageError>),
    /// An error that occurred while processing an SVG.
    #[error("svg error: {0}")]
    Usvg(Arc<usvg::Error>),
}

impl From<anyhow::Error> for ImageCacheError {
    fn from(value: anyhow::Error) -> Self {
        Self::Other(Arc::new(value))
    }
}

impl From<io::Error> for ImageCacheError {
    fn from(value: io::Error) -> Self {
        Self::Io(Arc::new(value))
    }
}

impl From<usvg::Error> for ImageCacheError {
    fn from(value: usvg::Error) -> Self {
        Self::Usvg(Arc::new(value))
    }
}

impl From<image::ImageError> for ImageCacheError {
    fn from(value: image::ImageError) -> Self {
        Self::Image(Arc::new(value))
    }
}

#[cfg(test)]
mod image_size_tests {
    use super::*;

    #[test]
    fn image_dimensions_can_be_checked_against_pixel_limits() {
        let pixels = ImageBuffer::from_pixel(32, 16, Rgba([0, 0, 0, 255]));
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(pixels)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();

        let svg_renderer = SvgRenderer::new(Arc::new(()));
        // The signature, IHDR and first IDAT header are sufficient; no pixel
        // data needs to be decoded before the max-size check.
        let header_only = &encoded.get_ref()[..41];
        let (width, height) = image_dimensions_from_bytes(
            header_only,
            None,
            &svg_renderer,
            None,
            None,
        )
        .unwrap();
        assert_eq!((width, height), (32, 16));
        assert!(!image_exceeds_limit(width, height, (32, 16)));
        assert!(image_exceeds_limit(width, height, (31, 16)));
    }
}
