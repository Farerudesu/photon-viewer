use crate::cache::SharedCache;
use crate::codec::{DecodedImage, LoadedFile};
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use rayon::ThreadPool;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

#[allow(dead_code)]
pub struct DecodeResult {
    pub path: PathBuf,
    pub image: Result<Arc<DecodedImage>, String>,
    pub generation: u64,
    pub is_active: bool,
}

pub struct PrefetchPipeline {
    cache: Arc<SharedCache>,
    thread_pool: ThreadPool,
    result_sender: Sender<DecodeResult>,
    result_receiver: Receiver<DecodeResult>,
    current_generation: Arc<AtomicU64>,
    active_cancellation: Arc<Mutex<Option<Arc<AtomicBool>>>>,
}

impl PrefetchPipeline {
    pub fn new(cache: Arc<SharedCache>) -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();

        // Dedicated decoding thread pool
        let num_threads = std::thread::available_parallelism()
            .map(|n| (n.get().saturating_sub(1)).max(2))
            .unwrap_or(4);

        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .thread_name(|i| format!("photon-decoder-{}", i))
            .build()
            .expect("Failed to build decode thread pool");

        Self {
            cache,
            thread_pool,
            result_sender: sender,
            result_receiver: receiver,
            current_generation: Arc::new(AtomicU64::new(0)),
            active_cancellation: Arc::new(Mutex::new(None)),
        }
    }

    pub fn next_generation(&self) -> (u64, Arc<AtomicBool>) {
        let mut cancel_guard = self.active_cancellation.lock();
        if let Some(ref old_cancel) = *cancel_guard {
            old_cancel.store(true, Ordering::SeqCst);
        }

        let new_gen = self.current_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let new_cancel = Arc::new(AtomicBool::new(false));
        *cancel_guard = Some(new_cancel.clone());

        (new_gen, new_cancel)
    }

    #[allow(dead_code)]
    pub fn request_active(&self, path: PathBuf) -> Option<Arc<DecodedImage>> {
        // Fast path: check LRU cache immediately
        if let Some(cached) = self.cache.get(&path) {
            return Some(cached);
        }

        let (generation, cancellation) = self.next_generation();
        let cache = self.cache.clone();
        let sender = self.result_sender.clone();
        let current_gen = self.current_generation.clone();

        self.thread_pool.spawn(move || {
            // Check cancellation before decoding
            if cancellation.load(Ordering::Relaxed)
                || current_gen.load(Ordering::Relaxed) != generation
            {
                return;
            }

            let result = match LoadedFile::open(&path) {
                Ok(file) => file.decode().map(Arc::new),
                Err(e) => Err(e),
            };

            // Check cancellation before caching and sending
            if cancellation.load(Ordering::Relaxed)
                || current_gen.load(Ordering::Relaxed) != generation
            {
                return;
            }

            if let Ok(ref img) = result {
                cache.insert(path.clone(), img.clone());
            }

            let _ = sender.send(DecodeResult {
                path,
                image: result,
                generation,
                is_active: true,
            });
        });

        None
    }

    pub fn cancel_all_pending(&self) {
        let mut cancel_guard = self.active_cancellation.lock();
        if let Some(ref old_cancel) = *cancel_guard {
            old_cancel.store(true, Ordering::SeqCst);
        }
        self.current_generation.fetch_add(1, Ordering::SeqCst);
        *cancel_guard = Some(Arc::new(AtomicBool::new(false)));
    }

    pub fn prefetch_neighbors(&self, files: &[PathBuf], current_index: usize, radius: usize) {
        if files.is_empty() {
            return;
        }

        let total = files.len();
        // Cancel previous prefetch generation so obsolete images are aborted immediately
        let (generation, cancellation) = self.next_generation();

        // Determine prefetch priority: N+1, N-1, N+2, N-2, ...
        let mut prefetch_indices = Vec::with_capacity(radius * 2);
        for step in 1..=radius {
            if current_index + step < total {
                prefetch_indices.push(current_index + step);
            }
            if current_index >= step {
                prefetch_indices.push(current_index - step);
            }
        }

        for idx in prefetch_indices {
            let path = files[idx].clone();

            // Skip if already in cache
            if self.cache.get(&path).is_some() {
                continue;
            }

            let cache = self.cache.clone();
            let sender = self.result_sender.clone();
            let current_gen = self.current_generation.clone();
            let cancel = cancellation.clone();

            self.thread_pool.spawn(move || {
                if cancel.load(Ordering::Relaxed)
                    || current_gen.load(Ordering::Relaxed) != generation
                {
                    return;
                }

                let result = match LoadedFile::open(&path) {
                    Ok(file) => file.decode().map(Arc::new),
                    Err(e) => Err(e),
                };

                if cancel.load(Ordering::Relaxed)
                    || current_gen.load(Ordering::Relaxed) != generation
                {
                    return;
                }

                if let Ok(ref img) = result {
                    cache.insert(path.clone(), img.clone());
                }

                let _ = sender.send(DecodeResult {
                    path,
                    image: result,
                    generation,
                    is_active: false,
                });
            });
        }
    }

    pub fn poll_results(&self) -> Vec<DecodeResult> {
        let mut results = Vec::new();
        while let Ok(res) = self.result_receiver.try_recv() {
            results.push(res);
        }
        results
    }
}
