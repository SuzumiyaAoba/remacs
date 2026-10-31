//! GNU Emacs 31.1's standard fringe bitmaps (src/fringe.c), in fringe.el order.
use super::render::GridFringeBitmapAlign;

pub(super) struct Bitmap {
    pub rows: &'static [u8],
    pub align: GridFringeBitmapAlign,
    pub repeat: bool,
}

const BITMAPS: &[Bitmap] = &[
    // question-mark
    Bitmap {
        rows: &[0x3c, 0x7e, 0xc3, 0xc3, 0x0c, 0x18, 0x18, 0x00, 0x18, 0x18],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // exclamation-mark
    Bitmap {
        rows: &[0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18, 0x18],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // left-arrow
    Bitmap {
        rows: &[0x18, 0x30, 0x60, 0xfc, 0xfc, 0x60, 0x30, 0x18],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // right-arrow
    Bitmap {
        rows: &[0x18, 0x0c, 0x06, 0x3f, 0x3f, 0x06, 0x0c, 0x18],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // up-arrow
    Bitmap {
        rows: &[0x18, 0x3c, 0x7e, 0xff, 0x18, 0x18, 0x18, 0x18],
        align: GridFringeBitmapAlign::Top,
        repeat: false,
    },
    // down-arrow
    Bitmap {
        rows: &[0x18, 0x18, 0x18, 0x18, 0xff, 0x7e, 0x3c, 0x18],
        align: GridFringeBitmapAlign::Bottom,
        repeat: false,
    },
    // left-curly-arrow
    Bitmap {
        rows: &[0x3c, 0x7c, 0xc0, 0xe4, 0xfc, 0x7c, 0x3c, 0x7c],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // right-curly-arrow
    Bitmap {
        rows: &[0x3c, 0x3e, 0x03, 0x27, 0x3f, 0x3e, 0x3c, 0x3e],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // large-circle
    Bitmap {
        rows: &[0x3c, 0x7e, 0xff, 0xff, 0xff, 0xff, 0x7e, 0x3c],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // left-triangle
    Bitmap {
        rows: &[0x03, 0x0f, 0x1f, 0x3f, 0x3f, 0x1f, 0x0f, 0x03],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // right-triangle
    Bitmap {
        rows: &[0xc0, 0xf0, 0xf8, 0xfc, 0xfc, 0xf8, 0xf0, 0xc0],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // top-left-angle
    Bitmap {
        rows: &[0xfc, 0xfc, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0x00],
        align: GridFringeBitmapAlign::Top,
        repeat: false,
    },
    // top-right-angle
    Bitmap {
        rows: &[0x3f, 0x3f, 0x03, 0x03, 0x03, 0x03, 0x03, 0x00],
        align: GridFringeBitmapAlign::Top,
        repeat: false,
    },
    // bottom-left-angle
    Bitmap {
        rows: &[0x00, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xfc, 0xfc],
        align: GridFringeBitmapAlign::Bottom,
        repeat: false,
    },
    // bottom-right-angle
    Bitmap {
        rows: &[0x00, 0x03, 0x03, 0x03, 0x03, 0x03, 0x3f, 0x3f],
        align: GridFringeBitmapAlign::Bottom,
        repeat: false,
    },
    // left-bracket
    Bitmap {
        rows: &[0xfc, 0xfc, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xfc, 0xfc],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // right-bracket
    Bitmap {
        rows: &[0x3f, 0x3f, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x3f, 0x3f],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // filled-rectangle
    Bitmap {
        rows: &[
            0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe,
        ],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // hollow-rectangle
    Bitmap {
        rows: &[
            0xfe, 0x82, 0x82, 0x82, 0x82, 0x82, 0x82, 0x82, 0x82, 0x82, 0x82, 0x82, 0xfe,
        ],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // filled-square
    Bitmap {
        rows: &[0x7e, 0x7e, 0x7e, 0x7e, 0x7e, 0x7e],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // hollow-square
    Bitmap {
        rows: &[0x7e, 0x42, 0x42, 0x42, 0x42, 0x7e],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // vertical-bar
    Bitmap {
        rows: &[
            0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0,
        ],
        align: GridFringeBitmapAlign::Center,
        repeat: false,
    },
    // horizontal-bar
    Bitmap {
        rows: &[0xfe, 0xfe],
        align: GridFringeBitmapAlign::Bottom,
        repeat: false,
    },
    // empty-line
    Bitmap {
        rows: &[0x00, 0x3c, 0x00],
        align: GridFringeBitmapAlign::Top,
        repeat: true,
    },
];

pub(super) fn get(index: usize) -> Option<&'static Bitmap> {
    BITMAPS.get(index.checked_sub(1)?)
}

pub(super) fn defined(index: usize) -> bool {
    get(index).is_some()
}
