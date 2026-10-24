use crate::tiling::grid::{DecodedTile, TileGrid, TileKey, TILE_SIZE};
use crossbeam_channel::{Receiver, Sender};
use std::collections::{HashMap, HashSet};
use std::thread;
use std::time::Instant;

pub const MAX_GPU_TILES: usize = 64;

pub struct GpuTile {
    #[allow(dead_code)]
    pub texture: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub last_used: Instant,
}

pub struct TileManager {
    grid: Option<TileGrid>,
    active_tiles: HashMap<TileKey, GpuTile>,
    pending_keys: HashSet<TileKey>,
    request_tx: Sender<TileKey>,
    result_rx: Receiver<DecodedTile>,
    _worker_handle: Option<thread::JoinHandle<()>>,
}

impl TileManager {
    pub fn new() -> Self {
        let (req_tx, _req_rx) = crossbeam_channel::unbounded::<TileKey>();
        let (_res_tx, res_rx) = crossbeam_channel::unbounded::<DecodedTile>();

        Self {
            grid: None,
            active_tiles: HashMap::with_capacity(MAX_GPU_TILES),
            pending_keys: HashSet::new(),
            request_tx: req_tx,
            result_rx: res_rx,
            _worker_handle: None,
        }
    }

    pub fn set_grid(&mut self, grid: TileGrid) {
        for (_, tile) in self.active_tiles.drain() {
            tile.texture.destroy();
        }
        self.pending_keys.clear();

        let (req_tx, req_rx) = crossbeam_channel::unbounded::<TileKey>();
        let (res_tx, res_rx) = crossbeam_channel::unbounded::<DecodedTile>();
        self.request_tx = req_tx;
        self.result_rx = res_rx;

        let grid_clone = grid.clone();
        let handle = thread::spawn(move || {
            while let Ok(key) = req_rx.recv() {
                if let Some(tile) = grid_clone.extract_tile(key) {
                    if res_tx.send(tile).is_err() {
                        break;
                    }
                }
            }
        });

        self.grid = Some(grid);
        self._worker_handle = Some(handle);
    }

    pub fn clear(&mut self) {
        self.grid = None;
        for (_, tile) in self.active_tiles.drain() {
            tile.texture.destroy();
        }
        self.pending_keys.clear();
    }

    pub fn is_active(&self) -> bool {
        self.grid.is_some()
    }

    /// Upload any newly decoded tiles arriving from background worker
    pub fn poll_new_tiles(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        sampler_linear: &wgpu::Sampler,
        sampler_nearest: &wgpu::Sampler,
    ) {
        while let Ok(tile) = self.result_rx.try_recv() {
            self.pending_keys.remove(&tile.key);

            let size = wgpu::Extent3d {
                width: tile.width,
                height: tile.height,
                depth_or_array_layers: 1,
            };

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Tile Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &tile.rgba,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * tile.width),
                    rows_per_image: Some(tile.height),
                },
                size,
            );

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Tile Bind Group"),
                layout: bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler_linear),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(sampler_nearest),
                    },
                ],
            });

            self.active_tiles.insert(
                tile.key,
                GpuTile {
                    texture,
                    bind_group,
                    u0: tile.u0,
                    v0: tile.v0,
                    u1: tile.u1,
                    v1: tile.v1,
                    last_used: Instant::now(),
                },
            );
        }
    }

    /// Calculate currently visible tiles and schedule background fetches
    pub fn update_visibility(
        &mut self,
        zoom: f32,
        pan_x: f32,
        pan_y: f32,
        vw: f32,
        vh: f32,
    ) -> Vec<TileKey> {
        let grid = match self.grid {
            Some(ref g) => g,
            None => return Vec::new(),
        };

        // When zoomed out, base overview proxy is sufficient
        if zoom < 0.6 {
            return Vec::new();
        }

        let iw = grid.full_width as f32;
        let ih = grid.full_height as f32;

        let center_x = vw * 0.5 + pan_x;
        let center_y = vh * 0.5 + pan_y;

        let screen_to_img_x = |sx: f32| -> f32 { (sx - center_x) / zoom + iw * 0.5 };
        let screen_to_img_y = |sy: f32| -> f32 { (sy - center_y) / zoom + ih * 0.5 };

        let x0 = screen_to_img_x(0.0).clamp(0.0, iw);
        let x1 = screen_to_img_x(vw).clamp(0.0, iw);
        let y0 = screen_to_img_y(0.0).clamp(0.0, ih);
        let y1 = screen_to_img_y(vh).clamp(0.0, ih);

        let min_x = x0.min(x1);
        let max_x = x0.max(x1);
        let min_y = y0.min(y1);
        let max_y = y0.max(y1);

        let col_start = (min_x as u32 / TILE_SIZE).saturating_sub(1);
        let col_end = ((max_x as u32 + TILE_SIZE - 1) / TILE_SIZE + 1).min(grid.cols);
        let row_start = (min_y as u32 / TILE_SIZE).saturating_sub(1);
        let row_end = ((max_y as u32 + TILE_SIZE - 1) / TILE_SIZE + 1).min(grid.rows);

        let mut visible_keys = Vec::new();
        let now = Instant::now();

        for r in row_start..row_end {
            for c in col_start..col_end {
                let key = TileKey { col: c, row: r };
                visible_keys.push(key);

                if let Some(gpu_tile) = self.active_tiles.get_mut(&key) {
                    gpu_tile.last_used = now;
                } else if !self.pending_keys.contains(&key) {
                    self.pending_keys.insert(key);
                    let _ = self.request_tx.send(key);
                }
            }
        }

        // Evict LRU tiles if exceeding budget
        if self.active_tiles.len() > MAX_GPU_TILES {
            let visible_set: HashSet<TileKey> = visible_keys.iter().copied().collect();
            let mut keys_to_evict: Vec<(TileKey, Instant)> = self
                .active_tiles
                .iter()
                .filter(|(k, _)| !visible_set.contains(k))
                .map(|(k, t)| (*k, t.last_used))
                .collect();

            keys_to_evict.sort_by_key(|&(_, time)| time);

            let excess = self.active_tiles.len().saturating_sub(MAX_GPU_TILES);
            for (k, _) in keys_to_evict.into_iter().take(excess) {
                if let Some(gpu_tile) = self.active_tiles.remove(&k) {
                    gpu_tile.texture.destroy();
                }
            }
        }

        visible_keys
    }

    pub fn get_tile(&self, key: &TileKey) -> Option<&GpuTile> {
        self.active_tiles.get(key)
    }

    pub fn stats(&self) -> (usize, usize, usize) {
        let (cols, rows) = self
            .grid
            .as_ref()
            .map(|g| (g.cols as usize, g.rows as usize))
            .unwrap_or((0, 0));
        (self.active_tiles.len(), self.pending_keys.len(), cols * rows)
    }
}
