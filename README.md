<div align="center">

# PHOTON VIEWER
**Ultra-fast, GPU-accelerated desktop image viewer engineered for instant opening of RAW camera photos, standard formats, and gigapixel imagery.**

[![Rust](https://img.shields.io/badge/Rust-2021%20Edition-DEA584?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%2F11-0078D6?style=for-the-badge&logo=windows)](https://microsoft.com)
[![Renderer](https://img.shields.io/badge/Renderer-WGPU%20(DX12%20%2F%20Vulkan)-orange?style=for-the-badge)](https://wgpu.rs)
[![License](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)](LICENSE)
[![Binary Size](https://img.shields.io/badge/Release%20Binary-11.7%20MB-blueviolet?style=for-the-badge)](#installation)

<br/>

<img src="assets/logo.png" width="128" height="128" alt="Photon Logo" />

</div>

---

## ⚡ Overview

**Photon Viewer** is built from the ground up in Rust to eliminate the sluggishness, bloat, and loading spinners common in modern desktop photo viewers. Powered by **WGPU** hardware rendering and **zero-copy memory mapping**, Photon opens multi-megabyte JPEG/PNGs in single-digit milliseconds and displays massive 45+ Megapixel RAW files in real-time.

```
+----------------------------------------------------------------------------------------------------+
|  [4/19] 2689595277.nef | RAW (Preview) | 8256x5504 | 245.9ms | Cache: 2 (175.4MB) | Bilinear       |
+----------------------------------------------------------------------------------------------------+
```

---

## 🚀 Key Features

### 🏎️ Instant Zero-Copy I/O & Parallel Prefetching
* **Memory-Mapped Files**: Reads file headers in under 0.2ms using `memmap2`, avoiding userspace buffer allocations.
* **Dual-Direction Speculative Prefetch**: Predicts sequential browsing forward and backward with a multi-threaded `Rayon` background queue.
* **LRU Memory-Bounded Cache**: Retains recent textures in GPU memory with configurable footprint limits (default 512 MB).

### 📷 High-Speed RAW Camera Decoder
* **SubIFD-Aware Embedded Previews**: Extracts full-resolution JPEG previews from TIFF SubIFDs (Tag `0x014A`) in ~0.1ms instead of falling back to low-resolution thumbnails.
* **Full Bayer Demosaicing**: Multi-threaded bilinear CFA demosaicing with normalized per-camera white balance coefficients for Sony ARW, Nikon NEF, Canon CR2/CR3, Fuji RAF, Olympus ORF, Panasonic RW2, and Adobe DNG.

### 🔬 Gigapixel Virtual Texture Tiling
* Seamless interactive navigation for gigantic resolutions exceeding GPU 2D texture dimensions (up to 64+ Megapixels).
* Dynamic Level-of-Detail (LOD) downsampled overview rendering with on-demand chunk uploading.

### 🎨 Retro-Futuristic Telemetry HUD
* Amber phosphor CRT telemetry display inspired by laboratory optical instruments.
* Real-time metadata readout: file index, format, pixel resolution, decode latency (microseconds), cache utilization, filtering mode, and tone-mapping status.

### 🖥️ Native Windows Integration
* **Single-Instance IPC**: Win32 Named Pipes guarantee instant folder browsing in a single unified window. Clicking images in File Explorer dynamically routes paths to the active viewer.
* **WinRAR-Style Setup**: Standalone `photon_setup.exe` installer for one-click file association, desktop shortcuts, and shell context menu integration.
* **Embedded Resource Icons**: Custom multi-resolution optical aperture icon baked directly into the `.exe` binary.

---

## 📊 Benchmark Highlights

Measured on a Windows 10/11 x64 desktop (NVMe SSD, AMD Ryzen / Intel Core):

| Format / Camera | File Size | Resolution | Cold Open to Pixels | Mode |
| :--- | :--- | :--- | :--- | :--- |
| **Nikon Z 8 (NEF)** | 31.9 MB | **8256 × 5504** (45.7 MP) | **245.9 ms** | SubIFD Preview |
| **Sony A7 IV (ARW)** | 34.2 MB | **6048 × 4024** (24.3 MP) | **280.4 ms** | Parallel CFA Demosaic |
| **Lightroom JPEG** | 12.4 MB | **6000 × 4000** (24.0 MP) | **32.8 ms** | Turbo Zune-JPEG |
| **High-Res PNG** | 8.6 MB | **3840 × 2160** (4K UHD) | **28.1 ms** | Chunked Stream Decoder |
| **HDR Radiance** | 4.8 MB | **2048 × 1024** | **14.2 ms** | Reinhard Tonemapped |
| **Vector SVG** | 1.2 MB | Scalable Vector | **9.6 ms** | Resvg GPU Raster |

---

## ⌨️ Controls & Shortcuts

| Input | Action |
| :--- | :--- |
| **Left Click + Drag** | Pan image viewport |
| **Mouse Wheel** | Smooth exponential zoom centered at cursor |
| **Double Click** | Quick zoom to 250% on focal point / Double click again to reset |
| **Right Arrow / D / Page Down** | Next image in directory (Natural Sort) |
| **Left Arrow / A / Page Up** | Previous image in directory |
| **Spacebar** | Fit image to window & center view |
| **Home / End** | Jump to first / last image |
| **B** | Toggle Texture Filtering (Bilinear Smoothing ⟷ Pixel-Art Nearest Neighbor) |
| **T** | Cycle HDR Tone Mapping (Off ⟷ Reinhard Operator) |
| **R** | Rotate image 90° clockwise |
| **H** | Toggle HUD telemetry overlay visibility |
| **Esc / Q** | Close viewer |

---

## 📁 Supported Formats

| Category | Formats |
| :--- | :--- |
| **Standard Raster** | JPEG, PNG, WebP, GIF (Animated), BMP, TIFF, TGA, ICO, QOI |
| **RAW Camera Formats** | Nikon (`.nef`), Sony (`.arw`), Canon (`.cr2`, `.cr3`), Fujifilm (`.raf`), Olympus (`.orf`), Panasonic (`.rw2`), Adobe Digital Negative (`.dng`) |
| **High Dynamic Range** | HDR (Radiance RGBE), OpenEXR (`.exr`) |
| **Graphics & Vector** | SVG, Photoshop PSD, DirectDraw Surface (DDS), PNM |

---

## 🛠️ Building from Source

### Prerequisites
* [Rust](https://rustup.rs/) (stable toolchain, 2021 edition)
* Windows 10 or 11 (DirectX 12 or Vulkan compatible GPU)
* MinGW `windres` (optional, for compiling embedded Windows `.ico` resources)

```bash
# Clone the repository
git clone https://github.com/your-username/photon-viewer.git
cd photon-viewer

# Build optimized release binaries
cargo build --release

# Run the viewer
cargo run --release --bin photon_viewer -- path/to/your/image.jpg

# Run the installer
cargo run --release --bin photon_setup
```

The resulting binaries will be located at:
* `target/release/photon_viewer.exe` (~11.7 MB standalone executable)
* `target/release/photon_setup.exe` (~12.1 MB zero-dependency installer)

---

## 📦 Project Structure

```
photon-viewer/
├── assets/
│   ├── banner.png          # High-resolution GitHub repository banner
│   ├── logo.png            # 1024x1024 Master brand mark
│   ├── logo.svg            # Scalable vector identity asset
│   └── icon.ico            # Windows multi-resolution PE icon (256..16px)
├── src/
│   ├── app.rs              # Application lifecycle, input & viewport controls
│   ├── build.rs            # Windows PE resource compilation (windres)
│   ├── cache.rs            # LRU image texture cache
│   ├── folder.rs           # Natural-sort directory scanner
│   ├── hud.rs              # CRT phosphor telemetry HUD renderer
│   ├── ipc.rs              # Win32 Named Pipe single-instance router
│   ├── main.rs             # Winit event loop & entry point
│   ├── pipeline/           # Asynchronous Rayon prefetch engine
│   ├── renderer/           # WGPU rendering pipeline & WGSL shaders
│   ├── tiling/             # Virtual texture tiling for gigapixel images
│   ├── bin/
│   │   └── setup.rs        # WinRAR-style association installer
│   └── codec/              # Format decoders (RAW, JPEG, PNG, HDR, TIFF, etc.)
└── tests/                  # Integration test suite (26 passing tests)
```

---

## 📄 License

Distributed under the **MIT License**. See `LICENSE` for more information.
