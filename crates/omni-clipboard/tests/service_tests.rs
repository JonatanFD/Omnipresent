use omni_clipboard::domain::{ClipboardData, ClipboardError, ClipboardImage};
use omni_clipboard::memory::MockClipboardBackend;
use omni_clipboard::service::ClipboardManager;

#[test]
fn test_clipboard_data_serialization() {
    let text_data = ClipboardData::Text("Hello World".to_string());
    let encoded_text = postcard::to_allocvec(&text_data).expect("serialize text");
    let decoded_text: ClipboardData =
        postcard::from_bytes(&encoded_text).expect("deserialize text");
    assert_eq!(text_data, decoded_text);

    let image_data = ClipboardData::Image(ClipboardImage {
        width: 2,
        height: 2,
        bytes: vec![255; 16],
    });
    let encoded_image = postcard::to_allocvec(&image_data).expect("serialize image");
    let decoded_image: ClipboardData =
        postcard::from_bytes(&encoded_image).expect("deserialize image");
    assert_eq!(image_data, decoded_image);
}

#[test]
fn test_manager_enforces_opt_in() {
    let mock = MockClipboardBackend::seeded(ClipboardData::Text("Initial content".to_string()));
    let manager = ClipboardManager::new(mock, false);

    let res = manager.poll_local_change();
    assert_eq!(res, Err(ClipboardError::Disabled));

    let res = manager.handle_remote_update(ClipboardData::Text("Remote update".to_string()));
    assert_eq!(res, Err(ClipboardError::Disabled));

    assert_eq!(
        manager.port().get_mock_data(),
        Some(ClipboardData::Text("Initial content".to_string()))
    );
}

#[test]
fn test_manager_detects_local_change() {
    let mock = MockClipboardBackend::new();
    let manager = ClipboardManager::new(mock, true);

    assert_eq!(manager.poll_local_change(), Ok(None));

    let copied = ClipboardData::Text("Changed!".to_string());
    manager.port().set_mock_data(copied.clone());

    assert_eq!(manager.poll_local_change(), Ok(Some(copied)));
    assert_eq!(manager.poll_local_change(), Ok(None));
}

#[test]
fn test_manager_prevents_feedback_loop() {
    let mock = MockClipboardBackend::new();
    let manager = ClipboardManager::new(mock, true);

    let remote_update = ClipboardData::Text("Synced".to_string());
    let res = manager.handle_remote_update(remote_update.clone());
    assert_eq!(res, Ok(()));

    assert_eq!(manager.port().get_mock_data(), Some(remote_update));
    assert_eq!(manager.poll_local_change(), Ok(None));
}

#[test]
fn test_image_dimension_validation() {
    let invalid_image = ClipboardImage {
        width: 2,
        height: 2,
        bytes: vec![255; 15],
    };
    assert!(invalid_image.validate().is_err());

    let valid_image = ClipboardImage {
        width: 2,
        height: 2,
        bytes: vec![255; 16],
    };
    assert!(valid_image.validate().is_ok());
}

#[test]
fn test_image_dimension_validation_overflow() {
    let invalid_image = ClipboardImage {
        width: u32::MAX,
        height: 4,
        bytes: vec![255; 16],
    };
    assert!(invalid_image.validate().is_err());
}

#[test]
fn test_manager_set_enabled_dynamic() {
    let mock = MockClipboardBackend::seeded(ClipboardData::Text("Content".to_string()));
    let manager = ClipboardManager::new(mock, false);

    assert_eq!(manager.poll_local_change(), Err(ClipboardError::Disabled));

    manager.set_enabled(true);
    assert!(manager.poll_local_change().is_ok());

    manager.set_enabled(false);
    assert_eq!(manager.poll_local_change(), Err(ClipboardError::Disabled));
}

#[test]
fn test_manager_reports_enabled_state() {
    let manager = ClipboardManager::new(MockClipboardBackend::new(), false);
    assert!(!manager.is_enabled());

    manager.set_enabled(true);
    assert!(manager.is_enabled());

    manager.set_enabled(false);
    assert!(!manager.is_enabled());
}
