use super::{grid_row_height, grid_row_top, style_colors, vscroll_pixels_for_cell};
use gpui::{Bounds, Pixels, Window, fill, point, px, size};
use remacs_runtime::frontend::{Grid, GridFringeBitmap, GridFringeBitmapAlign};

#[derive(Debug, PartialEq)]
struct BitmapRect {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    foreground: bool,
}

fn rectangles(
    bitmap: &GridFringeBitmap,
    height: f32,
    clip_top: f32,
    clip_bottom: f32,
) -> Vec<BitmapRect> {
    let width = bitmap.pixel_width as f32;
    if bitmap.bits.is_empty() || width == 0.0 || !height.is_finite() || height <= 0.0 {
        return Vec::new();
    }
    let clip_top = clip_top.max(0.0);
    let clip_bottom = clip_bottom.min(height);
    if clip_top >= clip_bottom {
        return Vec::new();
    }
    let left = ((width - 8.0) / 2.0).trunc();
    let source_height = bitmap.bits.len() as f32;
    let top = if bitmap.repeat {
        0.0
    } else {
        match bitmap.align {
            GridFringeBitmapAlign::Top => 0.0,
            GridFringeBitmapAlign::Bottom => height - source_height,
            GridFringeBitmapAlign::Center => ((height - source_height) / 2.0).trunc(),
        }
    };
    let mut result = Vec::new();
    let mut push = |x: f32, y: f32, w: f32, h: f32, foreground| {
        let right = (x + w).min(width);
        let bottom = (y + h).min(clip_bottom);
        let x = x.max(0.0);
        let y = y.max(clip_top);
        if x < right && y < bottom {
            result.push(BitmapRect {
                left: x,
                top: y,
                width: right - x,
                height: bottom - y,
                foreground,
            });
        }
    };
    push(
        left,
        top,
        8.0,
        if bitmap.repeat { height } else { source_height },
        false,
    );
    let rows = if bitmap.repeat {
        clip_top.floor() as usize..clip_bottom.ceil() as usize
    } else {
        0..bitmap.bits.len()
    };
    for row in rows {
        let bits = bitmap.bits[row % bitmap.bits.len()];
        let mut column = 0;
        while column < 8 {
            if bits & (0x80 >> column) == 0 {
                column += 1;
                continue;
            }
            let start = column;
            while column < 8 && bits & (0x80 >> column) != 0 {
                column += 1;
            }
            push(
                left + start as f32,
                top + row as f32,
                (column - start) as f32,
                1.0,
                true,
            );
        }
    }
    result
}

pub(super) fn paint(
    grid: &Grid,
    bounds: Bounds<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    default_background: u32,
    background_alpha: u8,
    window: &mut Window,
) {
    for bitmap in &grid.fringe_bitmaps {
        let row_height = grid_row_height(grid, bitmap.row, line_height);
        let origin = point(
            bounds.left() + cell_width * bitmap.column as f32,
            bounds.top() + grid_row_top(grid, bitmap.row, line_height)
                - px(vscroll_pixels_for_cell(grid, bitmap.row, bitmap.column) as f32),
        );
        let (foreground, background) = style_colors(
            bitmap.style,
            default_background,
            background_alpha,
            grid.gamma_exponent,
        );
        for rect in rectangles(
            bitmap,
            f32::from(row_height),
            f32::from(bounds.top() - origin.y),
            f32::from(bounds.bottom() - origin.y),
        ) {
            let color = if rect.foreground {
                Some(foreground)
            } else {
                background
            };
            if let Some(color) = color {
                window.paint_quad(fill(
                    Bounds {
                        origin: point(origin.x + px(rect.left), origin.y + px(rect.top)),
                        size: size(px(rect.width), px(rect.height)),
                    },
                    color,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GridFringeBitmap, GridFringeBitmapAlign, rectangles};
    use remacs_runtime::frontend::CellStyle;
    use std::sync::Arc;

    fn bitmap(align: GridFringeBitmapAlign) -> GridFringeBitmap {
        GridFringeBitmap {
            row: 0,
            column: 0,
            pixel_width: 8,
            bits: Arc::from([0b10000001, 0b00111100]),
            align,
            repeat: false,
            style: CellStyle::default(),
        }
    }

    #[test]
    fn bitmap_pixels_follow_bit_order_and_vertical_alignment() {
        for (align, top) in [
            (GridFringeBitmapAlign::Top, 0.0),
            (GridFringeBitmapAlign::Center, 3.0),
            (GridFringeBitmapAlign::Bottom, 7.0),
        ] {
            let rects = rectangles(&bitmap(align), 9.0, 0.0, 9.0);
            let pixels: Vec<_> = rects
                .iter()
                .filter(|rect| rect.foreground)
                .map(|rect| (rect.left, rect.top, rect.width, rect.height))
                .collect();
            assert_eq!(
                pixels,
                vec![
                    (0.0, top, 1.0, 1.0),
                    (7.0, top, 1.0, 1.0),
                    (2.0, top + 1.0, 4.0, 1.0)
                ]
            );
        }
    }

    #[test]
    fn narrow_fringes_clip_pixels_instead_of_scaling_them() {
        let mut bitmap = bitmap(GridFringeBitmapAlign::Center);
        bitmap.pixel_width = 4;
        let rects = rectangles(&bitmap, 1.0, 0.0, 1.0);
        assert_eq!(rects.iter().filter(|rect| rect.foreground).count(), 0);
        let rects = rectangles(&bitmap, 2.0, 0.0, 2.0);
        let pixels: Vec<_> = rects.iter().filter(|rect| rect.foreground).collect();
        assert_eq!(pixels.len(), 1);
        assert_eq!((pixels[0].left, pixels[0].width), (0.0, 4.0));
    }

    #[test]
    fn repeated_bitmaps_only_expand_the_visible_rows() {
        let mut bitmap = bitmap(GridFringeBitmapAlign::Top);
        bitmap.bits = Arc::from([0x80, 0x01, 0]);
        bitmap.repeat = true;
        let rects = rectangles(&bitmap, 1_000_000.0, 20.0, 25.0);
        let pixels: Vec<_> = rects
            .iter()
            .filter(|rect| rect.foreground)
            .map(|rect| (rect.left, rect.top))
            .collect();
        assert_eq!(pixels, vec![(0.0, 21.0), (7.0, 22.0), (0.0, 24.0)]);
        assert!(
            rects
                .iter()
                .all(|rect| rect.top >= 20.0 && rect.top + rect.height <= 25.0)
        );
    }
}
