use super::*;

#[test]
fn maps_external_file_drop_operations_to_lisp_actions() {
    assert_eq!(external_file_drop_action(ExternalDragOperation::Copy), "copy");
    assert_eq!(external_file_drop_action(ExternalDragOperation::Move), "move");
    assert_eq!(external_file_drop_action(ExternalDragOperation::Link), "link");
    assert_eq!(external_file_drop_action(ExternalDragOperation::Generic), "private");
}
