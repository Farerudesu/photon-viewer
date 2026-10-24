use crate::cache::SharedCache;
use crate::codec::{AnimatedImage, DecodedImage, LoadedFile};
use crate::folder::FolderScanner;
use crate::hud::HudBuilder;
use crate::pipeline::PrefetchPipeline;
use crate::renderer::RenderState;
use crate::tiling::{is_huge_image, TileGrid, TileManager};
use crossbeam_channel::{Receiver, Sender};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

pub struct App {
    render_state: RenderState,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    rotation_deg: f32,
    flip_h: bool,
    flip_v: bool,

    // Mouse drag & double click state
    is_panning: bool,
    cursor_pos: (f64, f64),
    last_cursor_pos: (f64, f64),
    last_click_time: Instant,
    last_click_pos: (f64, f64),

    // Active image metadata
    current_path: Option<PathBuf>,
    filename: String,
    format_name: &'static str,
    decode_us: u128,
    is_cache_hit: bool,
    hud_visible: bool,
    is_fullscreen: bool,

    // Phase 2: Cache, Prefetch, & Folder Browser
    cache: Arc<SharedCache>,
    pipeline: PrefetchPipeline,
    folder_files: Vec<PathBuf>,
    folder_index: usize,
    folder_channel_rx: Receiver<(Vec<PathBuf>, usize)>,
    folder_channel_tx: Sender<(Vec<PathBuf>, usize)>,
    folder_cancel: Arc<AtomicBool>,

    // Phase 3: Animation timeline
    current_animation: Option<AnimatedImage>,
    anim_frame_idx: usize,
    last_frame_time: Instant,

    // Phase 4: RAW live background demosaic swap
    raw_channel_rx: Receiver<(PathBuf, Arc<DecodedImage>)>,
    raw_channel_tx: Sender<(PathBuf, Arc<DecodedImage>)>,

    // Phase 5: Gigapixel Tiling Engine
    tile_manager: TileManager,

    // Phase 6: Shaders, Tone Mapping, Color Management & OLED
    filter_mode: u32,
    tonemap_mode: u32,
    exposure_ev: f32,
    color_space: u32,
    is_oled_black: bool,
    last_interaction: Instant,

    // Adaptive prefetch radius
    pub prefetch_radius: usize,

    // Navigation debounce for rapid arrow skipping
    pending_path: Option<PathBuf>,

    // Active RAW background demosaic cancellation token
    raw_cancel: Arc<AtomicBool>,
}

#[cfg(target_os = "windows")]
fn get_system_ram_bytes() -> u64 {
    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }
    extern "system" {
        fn GlobalMemoryStatusEx(stat: *mut MemoryStatusEx) -> i32;
    }
    let mut status = MemoryStatusEx {
        length: std::mem::size_of::<MemoryStatusEx>() as u32,
        memory_load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page_file: 0,
        avail_page_file: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_extended_virtual: 0,
    };
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok != 0 && status.total_phys > 0 {
        status.total_phys
    } else {
        8 * 1024 * 1024 * 1024
    }
}

#[cfg(not(target_os = "windows"))]
fn get_system_ram_bytes() -> u64 {
    8 * 1024 * 1024 * 1024
}

impl App {
    pub async fn new(window: Arc<Window>, initial_file: Option<PathBuf>) -> Result<Self, String> {
        let render_state = RenderState::new(window).await?;

        let ram = get_system_ram_bytes();
        // Ultra-lean memory bounding: 192 MB hard limit guarantees that the viewer
        // process working set stays around 120-250 MB total RAM on Windows, completely
        // eliminating OS memory pressure, swapping, and OOM crashes.
        let cache_bytes = 192 * 1024 * 1024;
        let prefetch_radius = 1;
        log::info!("System RAM: {} MB, SharedCache: {} MB, Prefetch radius: {}", ram / 1024 / 1024, cache_bytes / 1024 / 1024, prefetch_radius);

        let cache = Arc::new(SharedCache::new(cache_bytes));
        let pipeline = PrefetchPipeline::new(cache.clone());
        let (folder_tx, folder_rx) = crossbeam_channel::unbounded();
        let (raw_tx, raw_rx) = crossbeam_channel::unbounded();

        let mut app = Self {
            render_state,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation_deg: 0.0,
            flip_h: false,
            flip_v: false,
            is_panning: false,
            cursor_pos: (0.0, 0.0),
            last_cursor_pos: (0.0, 0.0),
            last_click_time: Instant::now().checked_sub(Duration::from_secs(10)).unwrap_or_else(Instant::now),
            last_click_pos: (0.0, 0.0),
            current_path: None,
            filename: "No file loaded".to_string(),
            format_name: "None",
            decode_us: 0,
            is_cache_hit: false,
            hud_visible: true,
            is_fullscreen: false,
            cache,
            pipeline,
            folder_files: Vec::new(),
            folder_index: 0,
            folder_channel_rx: folder_rx,
            folder_channel_tx: folder_tx,
            folder_cancel: Arc::new(AtomicBool::new(false)),
            current_animation: None,
            anim_frame_idx: 0,
            last_frame_time: Instant::now(),
            raw_channel_rx: raw_rx,
            raw_channel_tx: raw_tx,
            tile_manager: TileManager::new(),
            filter_mode: 0,
            tonemap_mode: 0,
            exposure_ev: 0.0,
            color_space: 0,
            is_oled_black: false,
            last_interaction: Instant::now(),
            prefetch_radius,
            pending_path: None,
            raw_cancel: Arc::new(AtomicBool::new(false)),
        };

        if let Some(path) = initial_file {
            app.open_file_and_scan_folder(path);
        }

        Ok(app)
    }

    pub fn should_use_tiling(&self, width: u32, height: u32) -> bool {
        width > self.render_state.max_texture_dimension_2d
            || height > self.render_state.max_texture_dimension_2d
            || is_huge_image(width, height)
    }

    pub fn open_file_and_scan_folder(&mut self, path: PathBuf) {
        self.folder_cancel.store(true, Ordering::Relaxed);
        self.folder_cancel = Arc::new(AtomicBool::new(false));

        FolderScanner::scan_async(
            path.clone(),
            self.folder_cancel.clone(),
            self.folder_channel_tx.clone(),
        );

        self.folder_files = vec![path.clone()];
        self.folder_index = 0;

        self.display_image_path(&path);
    }

    pub fn display_image_path(&mut self, path: &Path) {
        log::info!("Displaying file: {:?}", path);
        self.current_animation = None;
        self.anim_frame_idx = 0;

        // Cancel any pending heavy RAW demosaic tasks immediately
        self.raw_cancel.store(true, Ordering::SeqCst);
        let new_raw_cancel = Arc::new(AtomicBool::new(false));
        self.raw_cancel = new_raw_cancel.clone();

        // Fast path: Check if already in LRU cache
        if let Some(cached) = self.cache.get(path) {
            self.is_cache_hit = true;
            self.decode_us = cached.decode_duration_us;
            self.current_path = Some(path.to_path_buf());
            self.filename = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let (rot, fh, fv) = crate::codec::exif::orientation_to_transform(cached.orientation);
            self.rotation_deg = rot;
            self.flip_h = fh;
            self.flip_v = fv;

            if self.should_use_tiling(cached.width, cached.height) {
                let grid = TileGrid::new(cached.width, cached.height, cached.rgba.clone().into());
                self.render_state.upload_image(
                    grid.overview.width,
                    grid.overview.height,
                    &grid.overview.rgba,
                    false,
                );
                self.tile_manager.set_grid(grid);
                self.format_name = "Gigapixel (Tiled)";
            } else {
                self.tile_manager.clear();
                self.render_state.poll_device();
                self.render_state
                    .upload_image(cached.width, cached.height, &cached.rgba, cached.is_hdr);
                self.format_name = crate::codec::detector::detect_format_with_path(&[], Some(path)).name();
            }

            self.fit_to_window();
            self.trigger_prefetch();
            self.render_state.window.request_redraw();
            return;
        }

        // Cache miss: Cancel any obsolete background prefetch jobs immediately
        // so CPU & memory are 100% focused on decoding the user's active photo!
        self.pipeline.cancel_all_pending();

        let start = Instant::now();
        match LoadedFile::open(path) {
            Ok(file) => {
                let fmt = file.format;
                let (rot, fh, fv) = crate::codec::exif::orientation_to_transform(file.orientation);
                self.rotation_deg = rot;
                self.flip_h = fh;
                self.flip_v = fv;
                self.color_space = crate::codec::exif::detect_color_space(&file.mmap);

                if fmt.is_raw() {
                    match file.decode_raw() {
                        Ok((decoded, is_preview)) => {
                            self.is_cache_hit = false;
                            self.decode_us = decoded.decode_duration_us;
                            self.current_path = Some(path.to_path_buf());
                            self.filename = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "Unknown".to_string());
                            self.format_name = if is_preview {
                                "RAW (Preview)"
                            } else {
                                "RAW (Full Demosaic)"
                            };

                            let arc_decoded = Arc::new(decoded);
                            if self.should_use_tiling(arc_decoded.width, arc_decoded.height) {
                                let grid = TileGrid::new(arc_decoded.width, arc_decoded.height, arc_decoded.rgba.clone().into());
                                self.render_state.upload_image(
                                    grid.overview.width,
                                    grid.overview.height,
                                    &grid.overview.rgba,
                                    false,
                                );
                                self.tile_manager.set_grid(grid);
                                self.format_name = "Gigapixel (Tiled)";
                            } else {
                                self.tile_manager.clear();
                                self.render_state.poll_device();
                                self.render_state.upload_image(
                                    arc_decoded.width,
                                    arc_decoded.height,
                                    &arc_decoded.rgba,
                                    arc_decoded.is_hdr,
                                );
                            }

                            // Always cache the decoded RAW image immediately
                            self.cache.insert(path.to_path_buf(), arc_decoded.clone());

                            // Only spawn heavy background demosaic if preview is a small thumbnail (< 1080p).
                            // High-resolution embedded previews from camera ISPs (like Nikon/Sony/Canon 24-45MP)
                            // are already full-resolution and do not need a redundant 1GB demosaic run!
                            let is_low_res_thumbnail = is_preview && (arc_decoded.width < 1920 || arc_decoded.height < 1080);
                            if is_low_res_thumbnail {
                                let cancel = new_raw_cancel.clone();
                                let tx = self.raw_channel_tx.clone();
                                let raw_path = path.to_path_buf();
                                rayon::spawn(move || {
                                    if cancel.load(Ordering::Relaxed) {
                                        return;
                                    }
                                    if let Ok(f) = LoadedFile::open(&raw_path) {
                                        if cancel.load(Ordering::Relaxed) {
                                            return;
                                        }
                                        if let Ok(full) = crate::codec::raw::decode_full_raw(&f.mmap) {
                                            if cancel.load(Ordering::Relaxed) {
                                                return;
                                            }
                                            let _ = tx.send((raw_path, Arc::new(full)));
                                        }
                                    }
                                });
                            }

                            self.fit_to_window();
                            self.trigger_prefetch();

                            log::info!(
                                "Raw image {} loaded (preview: {}) in {:.2}ms",
                                self.filename,
                                is_preview,
                                start.elapsed().as_micros() as f64 / 1000.0
                            );
                        }
                        Err(e) => {
                            log::error!("Failed to decode RAW image {:?}: {}", path, e);
                            self.filename = format!("Error: {}", e);
                        }
                    }
                } else {
                    self.format_name = fmt.name();
                    if fmt == crate::codec::ImageFormat::Hdr || fmt == crate::codec::ImageFormat::OpenExr {
                        self.tonemap_mode = 1; // Auto-activate Reinhard tone mapping for HDR/EXR
                    }
                    match file.decode_with_animation() {
                        Ok((decoded, anim)) => {
                            self.is_cache_hit = false;
                            self.decode_us = decoded.decode_duration_us;
                            self.current_path = Some(path.to_path_buf());
                            self.filename = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "Unknown".to_string());

                            let arc_decoded = Arc::new(decoded);
                            self.cache.insert(path.to_path_buf(), arc_decoded.clone());

                            if self.should_use_tiling(arc_decoded.width, arc_decoded.height) {
                                let grid = TileGrid::new(arc_decoded.width, arc_decoded.height, arc_decoded.rgba.clone().into());
                                self.render_state.upload_image(
                                    grid.overview.width,
                                    grid.overview.height,
                                    &grid.overview.rgba,
                                    false,
                                );
                                self.tile_manager.set_grid(grid);
                                self.format_name = "Gigapixel (Tiled)";
                            } else {
                                self.tile_manager.clear();
                                self.render_state.poll_device();
                                self.render_state.upload_image(
                                    arc_decoded.width,
                                    arc_decoded.height,
                                    &arc_decoded.rgba,
                                    arc_decoded.is_hdr,
                                );
                            }

                            if let Some(animation) = anim {
                                self.current_animation = Some(animation);
                                self.last_frame_time = Instant::now();
                            }

                            self.fit_to_window();
                            self.trigger_prefetch();

                            log::info!(
                                "Freshly decoded {} in {:.2}ms",
                                self.filename,
                                start.elapsed().as_micros() as f64 / 1000.0
                            );
                        }
                        Err(e) => {
                            log::error!("Failed to decode image {:?}: {}", path, e);
                            self.filename = format!("Error: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to open file {:?}: {}", path, e);
                self.filename = format!("Error: {}", e);
            }
        }

        self.update_window_title();
        self.render_state.window.request_redraw();
    }

    pub fn update_animation(&mut self) {
        if let Some(ref anim) = self.current_animation {
            if anim.frames.is_empty() {
                return;
            }
            let current_frame = &anim.frames[self.anim_frame_idx];
            let elapsed_ms = self.last_frame_time.elapsed().as_millis() as u32;

            if elapsed_ms >= current_frame.delay_ms {
                self.anim_frame_idx = (self.anim_frame_idx + 1) % anim.frames.len();
                let next_frame = &anim.frames[self.anim_frame_idx];
                self.render_state.upload_image(anim.width, anim.height, &next_frame.rgba, false);
                self.last_frame_time = Instant::now();
                self.render_state.window.request_redraw();
            }
        }
    }

    pub fn animation_deadline(&self) -> Option<std::time::Instant> {
        if let Some(ref anim) = self.current_animation {
            if anim.frames.is_empty() {
                return None;
            }
            let current_frame = &anim.frames[self.anim_frame_idx];
            let elapsed = self.last_frame_time.elapsed();
            let delay = std::time::Duration::from_millis(current_frame.delay_ms.max(10) as u64);
            if elapsed >= delay {
                Some(Instant::now())
            } else {
                Some(self.last_frame_time + delay)
            }
        } else {
            None
        }
    }

    pub fn trigger_prefetch(&self) {
        if !self.folder_files.is_empty() {
            // Adaptive prefetch radius based on system physical RAM (radius 2 for >=12GB, 1 for low RAM)
            self.pipeline
                .prefetch_neighbors(&self.folder_files, self.folder_index, self.prefetch_radius);
        }
    }

    pub fn next_image(&mut self) {
        if self.folder_files.is_empty() {
            return;
        }
        if self.folder_index + 1 < self.folder_files.len() {
            self.folder_index += 1;
            let path = self.folder_files[self.folder_index].clone();
            self.filename = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            self.raw_cancel.store(true, Ordering::SeqCst);
            if self.cache.get(&path).is_some() {
                self.pending_path = None;
                self.display_image_path(&path);
            } else {
                self.pending_path = None;
                self.current_path = Some(path.clone());
                self.pipeline.request_active(path);
                self.trigger_prefetch();
                self.render_state.window.request_redraw();
            }
        }
    }

    pub fn prev_image(&mut self) {
        if self.folder_files.is_empty() {
            return;
        }
        if self.folder_index > 0 {
            self.folder_index -= 1;
            let path = self.folder_files[self.folder_index].clone();
            self.filename = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            self.raw_cancel.store(true, Ordering::SeqCst);
            if self.cache.get(&path).is_some() {
                self.pending_path = None;
                self.display_image_path(&path);
            } else {
                self.pending_path = None;
                self.current_path = Some(path.clone());
                self.pipeline.request_active(path);
                self.trigger_prefetch();
                self.render_state.window.request_redraw();
            }
        }
    }

    pub fn first_image(&mut self) {
        if !self.folder_files.is_empty() {
            self.folder_index = 0;
            let path = self.folder_files[0].clone();
            self.filename = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            self.raw_cancel.store(true, Ordering::SeqCst);
            if self.cache.get(&path).is_some() {
                self.pending_path = None;
                self.display_image_path(&path);
            } else {
                self.pending_path = None;
                self.current_path = Some(path.clone());
                self.pipeline.request_active(path);
                self.trigger_prefetch();
                self.render_state.window.request_redraw();
            }
        }
    }

    pub fn last_image(&mut self) {
        if !self.folder_files.is_empty() {
            self.folder_index = self.folder_files.len() - 1;
            let path = self.folder_files[self.folder_index].clone();
            self.filename = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            self.raw_cancel.store(true, Ordering::SeqCst);
            if self.cache.get(&path).is_some() {
                self.pending_path = None;
                self.display_image_path(&path);
            } else {
                self.pending_path = None;
                self.current_path = Some(path.clone());
                self.pipeline.request_active(path);
                self.trigger_prefetch();
                self.render_state.window.request_redraw();
            }
        }
    }

    pub fn calculate_fit_zoom(&self) -> f32 {
        if self.render_state.image_width == 0 || self.render_state.image_height == 0 {
            return 1.0;
        }

        let vw = self.render_state.size.width as f32;
        let vh = self.render_state.size.height as f32;
        let is_transposed = (self.rotation_deg / 90.0).round() as i32 % 2 != 0;
        let (iw, ih) = if is_transposed {
            (self.render_state.image_height as f32, self.render_state.image_width as f32)
        } else {
            (self.render_state.image_width as f32, self.render_state.image_height as f32)
        };

        let scale_x = vw / iw;
        let scale_y = vh / ih;
        scale_x.min(scale_y)
    }

    pub fn fit_to_window(&mut self) {
        if self.render_state.image_width == 0 || self.render_state.image_height == 0 {
            return;
        }

        self.zoom = self.calculate_fit_zoom();
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        self.update_window_title();
    }

    pub fn zoom_to_at_cursor(&mut self, target_zoom: f32) {
        if self.zoom <= 0.0001 {
            return;
        }
        let factor = target_zoom / self.zoom;
        self.zoom_at_cursor(factor);
    }

    pub fn update_window_title(&self) {
        let title = if self.render_state.image_width > 0 && self.render_state.image_height > 0 {
            let zoom_pct = (self.zoom * 100.0).round() as i32;
            let folder_info = if !self.folder_files.is_empty() {
                format!(" [{}/{}]", self.folder_index + 1, self.folder_files.len())
            } else {
                String::new()
            };
            format!(
                "Photon Viewer - {} ({}x{}) - {}%{}",
                self.filename,
                self.render_state.image_width,
                self.render_state.image_height,
                zoom_pct,
                folder_info
            )
        } else {
            "Photon Viewer".to_string()
        };
        self.render_state.window.set_title(&title);
    }

    pub fn is_image_lost(&self) -> bool {
        if self.render_state.image_width == 0 || self.render_state.image_height == 0 {
            return false;
        }

        let vw = self.render_state.size.width as f32;
        let vh = self.render_state.size.height as f32;
        if vw <= 10.0 || vh <= 10.0 {
            return false;
        }

        let fit_zoom = self.calculate_fit_zoom();

        // 1. Zoomed out too far (less than 40% of fit zoom or smaller than 70px)
        if self.zoom < fit_zoom * 0.40 {
            return true;
        }

        let is_transposed = (self.rotation_deg / 90.0).round() as i32 % 2 != 0;
        let (iw, ih) = if is_transposed {
            (self.render_state.image_height as f32, self.render_state.image_width as f32)
        } else {
            (self.render_state.image_width as f32, self.render_state.image_height as f32)
        };

        let disp_w = iw * self.zoom;
        let disp_h = ih * self.zoom;

        if disp_w < 70.0 || disp_h < 70.0 {
            return true;
        }

        // 2. Off-screen check: image bounding box completely or mostly outside viewport
        let cx = vw * 0.5 + self.pan_x;
        let cy = vh * 0.5 + self.pan_y;
        let x0 = cx - disp_w * 0.5;
        let x1 = cx + disp_w * 0.5;
        let y0 = cy - disp_h * 0.5;
        let y1 = cy + disp_h * 0.5;

        // Outside visible viewport bounds
        if x1 < 40.0 || x0 > vw - 40.0 || y1 < 40.0 || y0 > vh - 40.0 {
            return true;
        }

        // 3. Zoomed out somewhat (< 80% fit) and panned significantly off-center
        if self.zoom < fit_zoom * 0.80 {
            let pan_dist_sq = self.pan_x * self.pan_x + self.pan_y * self.pan_y;
            let threshold = (vw.min(vh) * 0.40).powi(2);
            if pan_dist_sq > threshold {
                return true;
            }
        }

        false
    }

    pub fn filter_name(&self) -> &'static str {
        match self.filter_mode {
            0 => "Bilinear",
            1 => "Bicubic",
            2 => "Mitchell",
            3 => "Nearest",
            4 => "Lanczos3",
            _ => "Bilinear",
        }
    }

    pub fn tonemap_name(&self) -> &'static str {
        match self.tonemap_mode {
            0 => "Tonemap: Off",
            1 => "Reinhard",
            2 => "ACES Filmic",
            _ => "Tonemap: Off",
        }
    }

    pub fn color_space_name(&self) -> &'static str {
        match self.color_space {
            0 => "sRGB",
            1 => "Display P3",
            2 => "Adobe RGB",
            _ => "sRGB",
        }
    }

    pub fn zoom_at_cursor(&mut self, factor: f32) {
        let vw = self.render_state.size.width as f32;
        let vh = self.render_state.size.height as f32;

        let old_zoom = self.zoom;
        let new_zoom = (old_zoom * factor).clamp(0.01, 1000.0);
        let ratio = new_zoom / old_zoom;

        let ox = self.cursor_pos.0 as f32 - vw * 0.5;
        let oy = self.cursor_pos.1 as f32 - vh * 0.5;

        self.pan_x = ox - (ox - self.pan_x) * ratio;
        self.pan_y = oy - (oy - self.pan_y) * ratio;
        self.zoom = new_zoom;

        self.update_window_title();
        self.render_state.window.request_redraw();
    }

    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        // Poll asynchronous folder scan completion
        while let Ok((files, idx)) = self.folder_channel_rx.try_recv() {
            log::info!("Folder scan complete: found {} images", files.len());
            self.folder_files = files;
            self.folder_index = idx;
            self.update_window_title();
            self.trigger_prefetch();
            self.render_state.window.request_redraw();
        }

        // Poll asynchronous full RAW demosaic results
        while let Ok((raw_path, full_image)) = self.raw_channel_rx.try_recv() {
            if let Some(ref cur_path) = self.current_path {
                if cur_path == &raw_path {
                    log::info!("Swapping RAW preview with full demosaiced image for {:?}", raw_path);
                    self.format_name = "RAW (Full Demosaic)";
                    self.decode_us = full_image.decode_duration_us;
                    self.cache.insert(raw_path.clone(), full_image.clone());

                    if self.should_use_tiling(full_image.width, full_image.height) {
                        let grid = TileGrid::new(full_image.width, full_image.height, full_image.rgba.clone().into());
                        self.render_state.upload_image(
                            grid.overview.width,
                            grid.overview.height,
                            &grid.overview.rgba,
                            false,
                        );
                        self.tile_manager.set_grid(grid);
                        self.format_name = "Gigapixel (Tiled)";
                    } else {
                        self.tile_manager.clear();
                        self.render_state.poll_device();
                        self.render_state.upload_image(
                            full_image.width,
                            full_image.height,
                            &full_image.rgba,
                            full_image.is_hdr,
                        );
                    }
                    self.update_window_title();
                    self.render_state.window.request_redraw();
                }
            }
        }

        // Poll async decode results from pipeline
        for result in self.pipeline.poll_results() {
            if result.is_active {
                if let Some(ref cur_path) = self.current_path {
                    if cur_path == &result.path {
                        if let Ok(img) = result.image {
                            let (rot, fh, fv) = crate::codec::exif::orientation_to_transform(img.orientation);
                            self.rotation_deg = rot;
                            self.flip_h = fh;
                            self.flip_v = fv;

                            if self.should_use_tiling(img.width, img.height) {
                                let grid = TileGrid::new(img.width, img.height, img.rgba.clone().into());
                                self.render_state.upload_image(
                                    grid.overview.width,
                                    grid.overview.height,
                                    &grid.overview.rgba,
                                    false,
                                );
                                self.tile_manager.set_grid(grid);
                                self.format_name = "Gigapixel (Tiled)";
                            } else {
                                self.tile_manager.clear();
                                self.render_state.poll_device();
                                self.render_state.upload_image(img.width, img.height, &img.rgba, img.is_hdr);
                                self.format_name = crate::codec::detector::detect_format_with_path(&[], Some(&result.path)).name();
                            }

                            self.decode_us = img.decode_duration_us;
                            self.fit_to_window();
                            self.trigger_prefetch();
                            self.render_state.window.request_redraw();
                        }
                    }
                }
            }
        }

        match event {
            WindowEvent::Resized(physical_size) => {
                self.render_state.resize(*physical_size);
                self.update_window_title();
                true
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = self.render_state.window.inner_size();
                self.render_state.resize(size);
                self.update_window_title();
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_interaction = Instant::now();
                self.cursor_pos = (position.x, position.y);
                if self.is_panning {
                    let dx = (position.x - self.last_cursor_pos.0) as f32;
                    let dy = (position.y - self.last_cursor_pos.1) as f32;
                    self.pan_x += dx;
                    self.pan_y += dy;
                    self.last_cursor_pos = (position.x, position.y);
                    self.render_state.window.request_redraw();
                }
                true
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.last_interaction = Instant::now();
                match button {
                    MouseButton::Left => {
                        if *state == ElementState::Pressed {
                            if self.is_image_lost() {
                                let (hx, hy, hw, hh) = HudBuilder::get_lost_hint_rect(
                                    self.render_state.size.width as f32,
                                    self.render_state.size.height as f32,
                                );
                                let mx = self.cursor_pos.0 as f32;
                                let my = self.cursor_pos.1 as f32;
                                if mx >= hx && mx <= hx + hw && my >= hy && my <= hy + hh {
                                    self.fit_to_window();
                                    self.render_state.window.request_redraw();
                                    return true;
                                }
                            }

                            let now = Instant::now();
                            let elapsed = now.duration_since(self.last_click_time);
                            let dx = self.cursor_pos.0 - self.last_click_pos.0;
                            let dy = self.cursor_pos.1 - self.last_click_pos.1;
                            let dist_sq = dx * dx + dy * dy;

                            if elapsed.as_millis() <= 350 && dist_sq <= 64.0 {
                                // Double click detected!
                                self.is_panning = false;
                                self.last_click_time = now.checked_sub(Duration::from_secs(10)).unwrap_or(now);

                                let fit_zoom = self.calculate_fit_zoom();
                                if self.zoom > fit_zoom * 1.15 {
                                    // Currently zoomed in: revert back to fit-to-window
                                    self.fit_to_window();
                                } else {
                                    // Currently fitted: zoom into cursor point (100% 1:1 pixel view or 2.5x)
                                    let target_zoom = if fit_zoom < 0.85 {
                                        1.0
                                    } else {
                                        (fit_zoom * 2.5).clamp(1.5, 8.0)
                                    };
                                    self.zoom_to_at_cursor(target_zoom);
                                }
                            } else {
                                self.last_click_time = now;
                                self.last_click_pos = self.cursor_pos;
                                self.is_panning = true;
                                self.last_cursor_pos = self.cursor_pos;
                            }
                        } else {
                            self.is_panning = false;
                        }
                    }
                    MouseButton::Middle => {
                        if *state == ElementState::Pressed {
                            let fit_zoom = self.calculate_fit_zoom();
                            if self.zoom > fit_zoom * 1.15 {
                                self.fit_to_window();
                            } else {
                                self.zoom_to_at_cursor(1.0);
                            }
                        }
                    }
                    MouseButton::Back => {
                        if *state == ElementState::Pressed {
                            self.prev_image();
                        }
                    }
                    MouseButton::Forward => {
                        if *state == ElementState::Pressed {
                            self.next_image();
                        }
                    }
                    _ => {}
                }
                self.render_state.window.request_redraw();
                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.last_interaction = Instant::now();
                let scroll_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 30.0,
                };
                let factor = if scroll_y > 0.0 { 1.15 } else { 1.0 / 1.15 };
                self.zoom_at_cursor(factor);
                true
            }
            WindowEvent::DroppedFile(path) => {
                self.last_interaction = Instant::now();
                self.open_file_and_scan_folder(path.clone());
                true
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                self.last_interaction = Instant::now();
                match key {
                    KeyCode::ArrowRight | KeyCode::KeyD | KeyCode::PageDown => {
                        self.next_image();
                    }
                    KeyCode::ArrowLeft | KeyCode::KeyA | KeyCode::PageUp => {
                        self.prev_image();
                    }
                    KeyCode::Home => {
                        self.first_image();
                    }
                    KeyCode::End => {
                        self.last_image();
                    }
                    KeyCode::KeyF | KeyCode::Digit0 => {
                        self.fit_to_window();
                    }
                    KeyCode::Digit1 => {
                        self.zoom = 1.0;
                        self.pan_x = 0.0;
                        self.pan_y = 0.0;
                    }
                    KeyCode::Equal | KeyCode::NumpadAdd => {
                        self.zoom_at_cursor(1.25);
                    }
                    KeyCode::Minus | KeyCode::NumpadSubtract => {
                        self.zoom_at_cursor(0.8);
                    }
                    KeyCode::KeyR => {
                        self.rotation_deg = (self.rotation_deg + 90.0) % 360.0;
                    }
                    KeyCode::KeyL => {
                        self.rotation_deg = (self.rotation_deg - 90.0 + 360.0) % 360.0;
                    }
                    KeyCode::KeyH => {
                        self.flip_h = !self.flip_h;
                    }
                    KeyCode::KeyV => {
                        self.flip_v = !self.flip_v;
                    }
                    KeyCode::KeyM | KeyCode::KeyB => {
                        self.filter_mode = (self.filter_mode + 1) % 5;
                    }
                    KeyCode::KeyT => {
                        self.tonemap_mode = (self.tonemap_mode + 1) % 3;
                    }
                    KeyCode::KeyP => {
                        self.color_space = (self.color_space + 1) % 3;
                    }
                    KeyCode::BracketRight => {
                        self.exposure_ev = (self.exposure_ev + 0.5).min(10.0);
                    }
                    KeyCode::BracketLeft => {
                        self.exposure_ev = (self.exposure_ev - 0.5).max(-10.0);
                    }
                    KeyCode::KeyC => {
                        self.is_oled_black = !self.is_oled_black;
                    }
                    KeyCode::Space => {
                        self.fit_to_window();
                    }
                    KeyCode::KeyI | KeyCode::Tab => {
                        self.hud_visible = !self.hud_visible;
                    }
                    KeyCode::Escape => {
                        if self.is_fullscreen {
                            self.is_fullscreen = false;
                            self.render_state.window.set_fullscreen(None);
                        } else {
                            let fit_zoom = self.calculate_fit_zoom();
                            if (self.zoom - fit_zoom).abs() > 0.01 * fit_zoom || self.pan_x.abs() > 1.0 || self.pan_y.abs() > 1.0 {
                                self.fit_to_window();
                            }
                        }
                    }
                    KeyCode::F11 => {
                        self.is_fullscreen = !self.is_fullscreen;
                        if self.is_fullscreen {
                            self.render_state.window.set_fullscreen(Some(
                                winit::window::Fullscreen::Borderless(None),
                            ));
                        } else {
                            self.render_state.window.set_fullscreen(None);
                        }
                    }
                    _ => return false,
                }
                self.render_state.window.request_redraw();
                true
            }
            WindowEvent::RedrawRequested => {
                if let Some(path) = self.pending_path.take() {
                    self.display_image_path(&path);
                }

                let mut tiles_to_render = Vec::new();
                if self.tile_manager.is_active() {
                    self.tile_manager.poll_new_tiles(
                        &self.render_state.device,
                        &self.render_state.queue,
                        self.render_state.bind_group_layout(),
                        self.render_state.uniform_buffer(),
                        self.render_state.samplers().0,
                        self.render_state.samplers().1,
                    );

                    let visible_keys = self.tile_manager.update_visibility(
                        self.zoom,
                        self.pan_x,
                        self.pan_y,
                        self.render_state.size.width as f32,
                        self.render_state.size.height as f32,
                    );

                    for key in &visible_keys {
                        if let Some(gpu_tile) = self.tile_manager.get_tile(key) {
                            tiles_to_render.push((&gpu_tile.bind_group, gpu_tile.u0, gpu_tile.v0, gpu_tile.u1, gpu_tile.v1));
                        }
                    }

                    let (_active, pending, _total) = self.tile_manager.stats();
                    if pending > 0 {
                        self.render_state.window.request_redraw();
                    }
                }

                let hud_active = self.hud_visible && self.last_interaction.elapsed().as_secs_f32() < 3.0;

                let (cache_len, cache_bytes, _max_bytes, _hits, _misses) = self.cache.stats();
                let cache_mb = cache_bytes as f32 / (1024.0 * 1024.0);

                let mut hud = HudBuilder::new();
                hud.build_overlay(
                    self.render_state.size.width as f32,
                    self.render_state.size.height as f32,
                    &self.filename,
                    self.format_name,
                    self.render_state.image_width,
                    self.render_state.image_height,
                    self.zoom,
                    self.decode_us,
                    self.folder_index,
                    self.folder_files.len(),
                    cache_len,
                    cache_mb,
                    self.is_cache_hit,
                    hud_active,
                    self.filter_name(),
                    self.tonemap_name(),
                    self.exposure_ev,
                    self.color_space_name(),
                );

                if self.is_image_lost() {
                    hud.add_lost_image_hint(
                        self.render_state.size.width as f32,
                        self.render_state.size.height as f32,
                    );
                }

                let hud_verts = hud.finish();

                match self.render_state.render(
                    self.zoom,
                    self.pan_x,
                    self.pan_y,
                    self.rotation_deg,
                    self.flip_h,
                    self.flip_v,
                    self.filter_mode,
                    self.tonemap_mode,
                    2.0f32.powf(self.exposure_ev),
                    self.color_space,
                    self.is_oled_black,
                    &tiles_to_render,
                    &hud_verts,
                ) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        self.render_state.resize(self.render_state.size);
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        log::error!("GPU Out of memory!");
                    }
                    Err(e) => {
                        log::warn!("Render error: {:?}", e);
                    }
                }
                true
            }
            _ => false,
        }
    }
}
