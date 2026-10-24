use photon_viewer::codec::exif::{orientation_to_transform, read_exif_orientation};
use photon_viewer::renderer::Uniforms;

#[test]
fn test_exif_orientation_transformations() {
    // 1: Normal
    assert_eq!(orientation_to_transform(1), (0.0, false, false));
    // 2: Flip Horizontal
    assert_eq!(orientation_to_transform(2), (0.0, true, false));
    // 3: Rotate 180
    assert_eq!(orientation_to_transform(3), (180.0, false, false));
    // 4: Flip Vertical
    assert_eq!(orientation_to_transform(4), (0.0, false, true));
    // 5: Transpose (Flip H + Rotate 90)
    assert_eq!(orientation_to_transform(5), (90.0, true, false));
    // 6: Rotate 90 CW
    assert_eq!(orientation_to_transform(6), (90.0, false, false));
    // 7: Transverse (Flip H + Rotate 270)
    assert_eq!(orientation_to_transform(7), (270.0, true, false));
    // 8: Rotate 270 CW
    assert_eq!(orientation_to_transform(8), (270.0, false, false));

    // Fallbacks for missing/invalid orientation tags
    assert_eq!(orientation_to_transform(0), (0.0, false, false));
    assert_eq!(orientation_to_transform(9), (0.0, false, false));
    assert_eq!(orientation_to_transform(65535), (0.0, false, false));
}

#[test]
fn test_exif_parser_little_endian() {
    // Construct a synthetic APP1 EXIF segment with Little-Endian (II) TIFF header
    let mut data = Vec::new();
    // SOI
    data.extend_from_slice(&[0xFF, 0xD8]);
    // APP1 Marker + Length
    data.extend_from_slice(&[0xFF, 0xE1]);
    let app1_len: u16 = 30;
    data.extend_from_slice(&app1_len.to_be_bytes());
    // "Exif\0\0"
    data.extend_from_slice(b"Exif\0\0");
    // TIFF header: "II" (0x49, 0x49), 42 (0x002A in LE: [0x2A, 0x00]), offset to IFD0 = 8 ([0x08, 0x00, 0x00, 0x00])
    data.extend_from_slice(b"II");
    data.extend_from_slice(&[0x2A, 0x00]);
    data.extend_from_slice(&[0x08, 0x00, 0x00, 0x00]);

    // IFD0: 1 tag
    data.extend_from_slice(&[0x01, 0x00]);
    // Tag 0x0112 (Orientation), type 3 (SHORT = [0x03, 0x00]), count 1 ([0x01, 0x00, 0x00, 0x00]), value 6 ([0x06, 0x00, 0x00, 0x00])
    data.extend_from_slice(&[0x12, 0x01]);
    data.extend_from_slice(&[0x03, 0x00]);
    data.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    data.extend_from_slice(&[0x06, 0x00, 0x00, 0x00]);

    let orientation = read_exif_orientation(&data);
    assert_eq!(orientation, 6);
    let (rot, fh, fv) = orientation_to_transform(orientation);
    assert_eq!(rot, 90.0);
    assert_eq!(fh, false);
    assert_eq!(fv, false);
}

#[test]
fn test_exif_parser_big_endian() {
    // Construct a synthetic APP1 EXIF segment with Big-Endian (MM) TIFF header
    let mut data = Vec::new();
    data.extend_from_slice(&[0xFF, 0xD8]);
    data.extend_from_slice(&[0xFF, 0xE1]);
    let app1_len: u16 = 30;
    data.extend_from_slice(&app1_len.to_be_bytes());
    data.extend_from_slice(b"Exif\0\0");
    // "MM", 42 ([0x00, 0x2A]), offset to IFD0 = 8 ([0x00, 0x00, 0x00, 0x08])
    data.extend_from_slice(b"MM");
    data.extend_from_slice(&[0x00, 0x2A]);
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x08]);

    // IFD0: 1 tag
    data.extend_from_slice(&[0x00, 0x01]);
    // Tag 0x0112, type 3, count 1, value 8 (Rotate 270 CW)
    data.extend_from_slice(&[0x01, 0x12]);
    data.extend_from_slice(&[0x00, 0x03]);
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
    data.extend_from_slice(&[0x00, 0x08, 0x00, 0x00]);

    let orientation = read_exif_orientation(&data);
    assert_eq!(orientation, 8);
    let (rot, fh, fv) = orientation_to_transform(orientation);
    assert_eq!(rot, 270.0);
    assert_eq!(fh, false);
    assert_eq!(fv, false);
}

#[test]
fn test_uniform_alignment_and_size() {
    let size = std::mem::size_of::<Uniforms>();
    assert_eq!(size, 112);
    // Strict 16-byte alignment requirement for WebGPU/WGSL uniform buffers
    assert_eq!(size % 16, 0);

    let u = Uniforms {
        transform: [[0.0; 4]; 4],
        viewport_size: [1920.0, 1080.0],
        image_size: [3840.0, 2160.0],
        zoom: 2.0,
        flags: 3,
        filter_mode: 4, // Lanczos3
        tonemap_mode: 2, // ACES
        exposure: 2.0f32.powf(1.5),
        color_space: 1, // Display P3
        _padding: [0.0, 0.0],
    };

    assert_eq!(u.filter_mode, 4);
    assert_eq!(u.tonemap_mode, 2);
    assert_eq!(u.color_space, 1);
    assert!((u.exposure - 2.8284).abs() < 0.001);
}
