use crate::{DevicePixels, Pixels, Result, SharedString, Size, size};
use smallvec::SmallVec;

use image::{Delay, Frame, Rgba};
use std::{
    borrow::Cow,
    fmt,
    hash::Hash,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
};

/// A source of assets for this app to use.
pub trait AssetSource: 'static + Send + Sync {
    /// Load the given asset from the source path.
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>>;

    /// List the assets at the given path.
    fn list(&self, path: &str) -> Result<Vec<SharedString>>;
}

impl AssetSource for () {
    fn load(&self, _path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(None)
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

/// A unique identifier for the image cache
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ImageId(pub usize);

#[derive(PartialEq, Eq, Hash, Clone)]
pub(crate) struct RenderImageParams {
    pub(crate) image_id: ImageId,
    pub(crate) frame_index: usize,
}

/// A cached and processed image, in BGRA format
pub struct RenderImage {
    /// The ID associated with this image
    pub id: ImageId,
    /// The scale factor of this image on render.
    pub(crate) scale_factor: f32,
    data: SmallVec<[Frame; 1]>,
}

impl PartialEq for RenderImage {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for RenderImage {}

impl RenderImage {
    /// Create a new image from the given data.
    pub fn new(data: impl Into<SmallVec<[Frame; 1]>>) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        Self {
            id: ImageId(NEXT_ID.fetch_add(1, SeqCst)),
            scale_factor: 1.0,
            data: data.into(),
        }
    }

    /// Convert this image into a byte slice.
    pub fn as_bytes(&self, frame_index: usize) -> Option<&[u8]> {
        self.data
            .get(frame_index)
            .map(|frame| frame.buffer().as_raw().as_slice())
    }

    /// Get the size of this image, in pixels.
    pub fn size(&self, frame_index: usize) -> Size<DevicePixels> {
        let (width, height) = self.data[frame_index].buffer().dimensions();
        size(width.into(), height.into())
    }

    /// Get the size of this image, in pixels for display, adjusted for the scale factor.
    pub(crate) fn render_size(&self, frame_index: usize) -> Size<Pixels> {
        self.size(frame_index)
            .map(|v| (v.0 as f32 / self.scale_factor).into())
    }

    /// Get the delay of this frame from the previous
    pub fn delay(&self, frame_index: usize) -> Delay {
        self.data[frame_index].delay()
    }

    /// Get the number of frames for this image.
    pub fn frame_count(&self) -> usize {
        self.data.len()
    }

    /// Apply GNU Emacs' disabled-image grayscale and contrast adjustment.
    pub(crate) fn disabled_conversion(&self) -> Self {
        const LOW: i64 = 30_000;
        const RANGE: i64 = 65_535 - 15_000 - 30_000;

        let mut frames = self.data.clone();
        for frame in &mut frames {
            let source = frame.buffer();
            let mut converted = source.clone();
            for (source, destination) in source.pixels().zip(converted.pixels_mut()) {
                let red = i64::from(source.0[2]) * 257;
                let green = i64::from(source.0[1]) * 257;
                let blue = i64::from(source.0[0]) * 257;
                let intensity = (2 * red + 3 * green + blue) / 6;
                let gray = ((RANGE * intensity / 65_535 + LOW) >> 8) as u8;
                *destination = Rgba([gray, gray, gray, source.0[3]]);
            }
            *frame.buffer_mut() = converted;
        }

        let mut result = Self::new(frames);
        result.scale_factor = self.scale_factor;
        result
    }

    /// Apply GNU Emacs' 3×3 edge-detection conversion to every frame.
    pub(crate) fn edge_detected(&self, matrix: [i32; 9], color_adjust: i32) -> Self {
        let sum_abs = matrix
            .iter()
            .map(|factor| i64::from(*factor).abs())
            .sum::<i64>();
        let mut frames = self.data.clone();
        if sum_abs > 0 {
            for frame in &mut frames {
                let source = frame.buffer().clone();
                let (width, height) = source.dimensions();
                let mut converted = source.clone();
                for y in 0..height {
                    for x in 0..width {
                        let alpha = source.get_pixel(x, y).0[3];
                        if x == 0 || y == 0 || x + 1 == width || y + 1 == height {
                            converted.put_pixel(x, y, Rgba([127, 127, 127, alpha]));
                            continue;
                        }
                        let mut sums = [0i64; 3];
                        for dy in 0..3usize {
                            for dx in 0..3usize {
                                let weight = i64::from(matrix[dy * 3 + dx]);
                                if weight == 0 {
                                    continue;
                                }
                                let pixel = source
                                    .get_pixel(x + dx as u32 - 1, y + dy as u32 - 1)
                                    .0;
                                // GPUI stores decoded image bytes in BGRA order.
                                sums[0] += weight * i64::from(pixel[2]) * 257;
                                sums[1] += weight * i64::from(pixel[1]) * 257;
                                sums[2] += weight * i64::from(pixel[0]) * 257;
                            }
                        }
                        let adjusted = sums.map(|sum| {
                            (sum / sum_abs + i64::from(color_adjust)) & 0xffff
                        });
                        let intensity = (2 * adjusted[0] + 3 * adjusted[1] + adjusted[2]) / 6;
                        let gray = (intensity >> 8).clamp(0, 255) as u8;
                        converted.put_pixel(x, y, Rgba([gray, gray, gray, alpha]));
                    }
                }
                *frame.buffer_mut() = converted;
            }
        }
        let mut result = Self::new(frames);
        result.scale_factor = self.scale_factor;
        result
    }
}

impl fmt::Debug for RenderImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageData")
            .field("id", &self.id)
            .field("size", &self.size(0))
            .finish()
    }
}

#[cfg(test)]
mod image_conversion_tests {
    use super::*;
    use image::ImageBuffer;

    #[test]
    fn edge_detection_transforms_pixels_and_preserves_alpha() {
        let mut pixels = ImageBuffer::from_pixel(3, 3, Rgba([0, 0, 0, 255]));
        pixels.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        let source = RenderImage::new(vec![Frame::new(pixels)]);

        let converted = source.edge_detected([1, 0, 0, 0, 0, 0, 0, 0, -1], 45_000);
        let bytes = converted.as_bytes(0).unwrap();
        assert_eq!(&bytes[..4], &[127, 127, 127, 255]);
        assert!(bytes[16] > 0);
        assert_eq!(bytes[16], bytes[17]);
        assert_eq!(bytes[17], bytes[18]);
        assert_eq!(bytes[19], 255);
    }

    #[test]
    fn disabled_conversion_grayscales_and_equalizes_pixels() {
        let mut pixels = ImageBuffer::from_pixel(2, 1, Rgba([0, 0, 0, 255]));
        pixels.put_pixel(1, 0, Rgba([255, 255, 255, 128]));
        let source = RenderImage::new(vec![Frame::new(pixels)]);

        let converted = source.disabled_conversion();
        let bytes = converted.as_bytes(0).unwrap();
        assert_eq!(&bytes[..4], &[117, 117, 117, 255]);
        assert_eq!(&bytes[4..8], &[197, 197, 197, 128]);
    }
}
