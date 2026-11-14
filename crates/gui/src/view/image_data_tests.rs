use super::grid_data_image;
use std::sync::Arc;

#[test]
fn portable_bitmap_data_is_decoded_by_the_gui_image_path() {
    let bytes: Arc<[u8]> = Arc::from(&b"P1\n2 2\n0 1\n1 0\n"[..]);
    assert!(grid_data_image("pbm".to_owned(), bytes, None, None).is_some());
}
