use photon_viewer::tiling::{is_huge_image, TileGrid, TileKey, TileManager, TILE_SIZE};

#[test]
fn test_huge_image_detection() {
    // Normal photos (< 64 MP and < 8192)
    assert!(!is_huge_image(1920, 1080));
    assert!(!is_huge_image(6000, 4000)); // 24 MP standard camera
    assert!(!is_huge_image(8192, 5464)); // 44.7 MP full frame

    // Huge images (panoramas, gigapixels, medium format scans)
    assert!(is_huge_image(10000, 8000)); // 80 MP
    assert!(is_huge_image(12000, 6000)); // 72 MP
    assert!(is_huge_image(9000, 2000));  // Width exceeds 8192 limit
    assert!(is_huge_image(2000, 9000));  // Height exceeds 8192 limit
    assert!(is_huge_image(25000, 20000)); // 500 Megapixel gigapixel scan
}

#[test]
fn test_tile_grid_generation_and_overview() {
    let width = 2048u32;
    let height = 1536u32;
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    // Stamp pattern on pixel buffer
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            pixels[idx] = (x % 256) as u8;
            pixels[idx + 1] = (y % 256) as u8;
            pixels[idx + 2] = 128;
            pixels[idx + 3] = 255;
        }
    }

    let grid = TileGrid::new(width, height, pixels);

    // Verify grid dimensions
    let expected_cols = (width + TILE_SIZE - 1) / TILE_SIZE; // (2048 + 511)/512 = 4
    let expected_rows = (height + TILE_SIZE - 1) / TILE_SIZE; // (1536 + 511)/512 = 3
    assert_eq!(grid.cols, expected_cols);
    assert_eq!(grid.rows, expected_rows);

    // Verify overview proxy
    assert!(grid.overview.width <= 2048);
    assert!(grid.overview.height <= 2048);
    assert_eq!(grid.overview.rgba.len(), (grid.overview.width * grid.overview.height * 4) as usize);

    // Extract tile (0, 0)
    let tile0 = grid.extract_tile(TileKey { col: 0, row: 0 }).expect("Failed to extract tile (0, 0)");
    assert_eq!(tile0.width, 512);
    assert_eq!(tile0.height, 512);
    assert_eq!(tile0.u0, 0.0);
    assert_eq!(tile0.v0, 0.0);
    assert_eq!(tile0.u1, 512.0 / 2048.0);
    assert_eq!(tile0.v1, 512.0 / 1536.0);
    assert_eq!(tile0.rgba.len(), 512 * 512 * 4);

    // Check pixel contents of tile (0, 0)
    assert_eq!(tile0.rgba[0], 0); // x=0 % 256
    assert_eq!(tile0.rgba[1], 0); // y=0 % 256
    assert_eq!(tile0.rgba[2], 128);
    assert_eq!(tile0.rgba[3], 255);

    // Extract edge tile (3, 2)
    let tile_edge = grid.extract_tile(TileKey { col: 3, row: 2 }).expect("Failed to extract edge tile");
    assert_eq!(tile_edge.width, 512);
    assert_eq!(tile_edge.height, 512);
    assert_eq!(tile_edge.u1, 1.0);
    assert_eq!(tile_edge.v1, 1.0);
}

#[test]
fn test_tile_manager_viewport_visibility() {
    let width = 4096u32;
    let height = 4096u32;
    let pixels = vec![200u8; (width * height * 4) as usize];
    let grid = TileGrid::new(width, height, pixels);

    let mut manager = TileManager::new();
    manager.set_grid(grid);

    // Case 1: Zoomed out (zoom = 0.4) -> No high-res tiles needed, overview is sharp
    let visible_zoomed_out = manager.update_visibility(0.4, 0.0, 0.0, 1920.0, 1080.0);
    assert!(visible_zoomed_out.is_empty(), "Zoomed-out state should use overview and not fetch tiles");

    // Case 2: Zoomed in to 2.0x, centered at (0, 0)
    let visible_zoomed_in = manager.update_visibility(2.0, 0.0, 0.0, 1920.0, 1080.0);
    assert!(!visible_zoomed_in.is_empty(), "Zoomed-in state must return visible tiles");

    // Visible tiles must be a strict subset of total tiles (cols=8, rows=8, total=64)
    assert!(visible_zoomed_in.len() < 64);

    // Verify center tiles are in the visible set
    let center_key = TileKey { col: 4, row: 4 };
    assert!(
        visible_zoomed_in.contains(&center_key),
        "Center tile must be visible when viewing center of image"
    );

    // Case 3: Pan far to the top-left corner
    let pan_left_visible = manager.update_visibility(2.0, 2000.0, 2000.0, 1920.0, 1080.0);
    let top_left_key = TileKey { col: 0, row: 0 };
    assert!(
        pan_left_visible.contains(&top_left_key),
        "Top-left tile (0, 0) must be visible when panned to top-left corner"
    );
}
