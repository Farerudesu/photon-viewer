use photon_viewer::cache::{LruMemoryCache, SharedCache};
use photon_viewer::codec::DecodedImage;
use photon_viewer::hud::HudBuilder;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn test_cache_remove() {
    let mut cache = LruMemoryCache::new(1024 * 1024);
    let path1 = PathBuf::from("test_img_1.jpg");
    let path2 = PathBuf::from("test_img_2.jpg");

    let img1 = Arc::new(DecodedImage {
        width: 10,
        height: 10,
        rgba: vec![0u8; 400],
        decode_duration_us: 10,
        orientation: 1,
        is_hdr: false,
    });
    let img2 = Arc::new(DecodedImage {
        width: 20,
        height: 20,
        rgba: vec![0u8; 1600],
        decode_duration_us: 20,
        orientation: 1,
        is_hdr: false,
    });

    cache.insert(path1.clone(), img1);
    cache.insert(path2.clone(), img2);
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.current_bytes(), 2000);

    // Remove path1
    let removed = cache.remove(&path1);
    assert!(removed.is_some());
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.current_bytes(), 1600);
    assert!(cache.get(&path1).is_none());
    assert!(cache.get(&path2).is_some());

    // Remove non-existent path
    let not_found = cache.remove(&path1);
    assert!(not_found.is_none());
    assert_eq!(cache.len(), 1);

    // SharedCache remove
    let shared = SharedCache::new(1024 * 1024);
    shared.insert(path2.clone(), Arc::new(DecodedImage {
        width: 5,
        height: 5,
        rgba: vec![0u8; 100],
        decode_duration_us: 5,
        orientation: 1,
        is_hdr: false,
    }));
    assert!(shared.get(&path2).is_some());
    shared.remove(&path2);
    assert!(shared.get(&path2).is_none());
}

#[test]
fn test_delete_dialog_rects_and_hud() {
    let screen_w = 1920.0;
    let screen_h = 1080.0;
    let rects = HudBuilder::get_delete_dialog_rects(screen_w, screen_h);

    // Dialog should be centered
    assert!(rects.dialog.0 > 0.0);
    assert!(rects.dialog.1 > 0.0);
    assert_eq!(rects.dialog.2, 480.0);
    assert_eq!(rects.dialog.3, 210.0);

    // Buttons must be inside dialog bounds
    assert!(rects.delete_btn.0 >= rects.dialog.0);
    assert!(rects.delete_btn.1 >= rects.dialog.1);
    assert!(rects.cancel_btn.0 + rects.cancel_btn.2 <= rects.dialog.0 + rects.dialog.2);

    // Checkbox inside dialog bounds
    assert!(rects.checkbox.0 >= rects.dialog.0);
    assert!(rects.checkbox.1 >= rects.dialog.1);

    // Vertex generation without panic
    let mut hud = HudBuilder::new();
    hud.add_delete_confirmation_dialog(
        screen_w,
        screen_h,
        "sample_vacation_photo.jpg",
        false,
        false,
        false,
        false,
    );
    hud.add_toast_notification(screen_w, screen_h, "Deleted: sample.jpg (Recycle Bin)");
    let verts = hud.finish();
    assert!(!verts.is_empty());
}

#[test]
fn test_trash_recycle_and_fallback() {
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join(format!("photon_test_delete_{}.tmp", std::process::id()));

    {
        let mut f = File::create(&test_file).expect("Failed to create temporary test file");
        writeln!(f, "test trash recycling").expect("Failed to write to test file");
    }
    assert!(test_file.exists());

    let res = trash::delete(&test_file);
    if let Err(e) = res {
        // Fallback to remove_file
        println!("trash::delete failed in test environment ({}), testing std::fs::remove_file fallback", e);
        std::fs::remove_file(&test_file).expect("Fallback remove_file failed");
    }
    assert!(!test_file.exists());
}
