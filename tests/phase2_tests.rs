use photon_viewer::cache::LruMemoryCache;
use photon_viewer::codec::DecodedImage;
use photon_viewer::folder::scanner::natural_sort_cmp;
use photon_viewer::pipeline::PrefetchPipeline;
use photon_viewer::cache::SharedCache;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn test_natural_sort_cmp() {
    let mut files = vec![
        PathBuf::from("photo10.jpg"),
        PathBuf::from("photo1.jpg"),
        PathBuf::from("photo2.jpg"),
        PathBuf::from("photo20.jpg"),
        PathBuf::from("PHOTO3.JPG"),
    ];

    files.sort_by(|a, b| natural_sort_cmp(a, b));

    assert_eq!(files[0], PathBuf::from("photo1.jpg"));
    assert_eq!(files[1], PathBuf::from("photo2.jpg"));
    assert_eq!(files[2], PathBuf::from("PHOTO3.JPG"));
    assert_eq!(files[3], PathBuf::from("photo10.jpg"));
    assert_eq!(files[4], PathBuf::from("photo20.jpg"));
}

#[test]
fn test_lru_cache_eviction() {
    // 100 KB memory limit
    let max_bytes = 100 * 1024;
    let mut cache = LruMemoryCache::new(max_bytes);

    // Each 100x100 RGBA image is 40,000 bytes (~39 KB)
    let make_img = || {
        Arc::new(DecodedImage {
            width: 100,
            height: 100,
            rgba: vec![0u8; 40_000],
            decode_duration_us: 100,
            orientation: 1,
        })
    };

    let p1 = PathBuf::from("img1.png");
    let p2 = PathBuf::from("img2.png");
    let p3 = PathBuf::from("img3.png");

    cache.insert(p1.clone(), make_img()); // 40,000 bytes
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.current_bytes(), 40_000);

    cache.insert(p2.clone(), make_img()); // 80,000 bytes
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.current_bytes(), 80_000);

    // Access p1 so p2 becomes LRU
    assert!(cache.get(&p1).is_some());

    // Insert p3 (40,000 bytes) -> total would be 120,000 > 100,000 -> must evict p2!
    cache.insert(p3.clone(), make_img());
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.current_bytes(), 80_000);

    // p1 should still be present, p2 should have been evicted
    assert!(cache.get(&p1).is_some());
    assert!(cache.get(&p2).is_none());
    assert!(cache.get(&p3).is_some());
}

#[test]
fn test_prefetch_pipeline_background_loading() {
    use image::{ImageBuffer, Rgba};
    use std::io::Cursor;

    let temp_dir = std::env::temp_dir().join("photon_prefetch_test");
    let _ = std::fs::create_dir_all(&temp_dir);

    // Create 3 real test images
    let mut file_paths = Vec::new();
    for i in 1..=3 {
        let path = temp_dir.join(format!("test_img_{}.png", i));
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(64, 64, Rgba([i as u8 * 50, 100, 150, 255]));
        let mut encoded = Vec::new();
        img.write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Png).unwrap();
        std::fs::write(&path, encoded).unwrap();
        file_paths.push(path);
    }

    let cache = Arc::new(SharedCache::new(50 * 1024 * 1024));
    let pipeline = PrefetchPipeline::new(cache.clone());

    // Currently viewing image 0, trigger prefetch for neighbors
    pipeline.prefetch_neighbors(&file_paths, 0, 2);

    // Wait briefly for worker thread pool to decode in background
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Poll pipeline results
    let _ = pipeline.poll_results();

    // Check that prefetch loaded neighbor images into LRU cache
    let cached_neighbor = cache.get(&file_paths[1]);
    assert!(cached_neighbor.is_some(), "Neighbor image should be populated in cache");
    assert_eq!(cached_neighbor.unwrap().width, 64);

    // Cleanup
    for p in file_paths {
        let _ = std::fs::remove_file(p);
    }
    let _ = std::fs::remove_dir(temp_dir);
}
