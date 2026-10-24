use crate::codec::DecodedImage;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct CacheEntry {
    image: Arc<DecodedImage>,
    size_bytes: usize,
    prev: Option<usize>,
    next: Option<usize>,
    path: PathBuf,
}

pub struct LruMemoryCache {
    entries: Vec<Option<CacheEntry>>,
    map: HashMap<PathBuf, usize>,
    free_slots: Vec<usize>,
    head: Option<usize>, // Most recently used
    tail: Option<usize>, // Least recently used
    current_bytes: usize,
    max_bytes: usize,
    pub hits: u64,
    pub misses: u64,
}

impl LruMemoryCache {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            entries: Vec::with_capacity(64),
            map: HashMap::with_capacity(64),
            free_slots: Vec::new(),
            head: None,
            tail: None,
            current_bytes: 0,
            max_bytes,
            hits: 0,
            misses: 0,
        }
    }

    pub fn get(&mut self, path: &Path) -> Option<Arc<DecodedImage>> {
        if let Some(&index) = self.map.get(path) {
            self.move_to_front(index);
            self.hits += 1;
            let entry = self.entries[index].as_ref().unwrap();
            Some(entry.image.clone())
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn insert(&mut self, path: PathBuf, image: Arc<DecodedImage>) {
        let size_bytes = (image.width as usize) * (image.height as usize) * 4;

        // If a single image is larger than the entire cache, don't store it
        if size_bytes > self.max_bytes {
            return;
        }

        // If entry already exists, update it
        if let Some(&index) = self.map.get(&path) {
            let old_size = self.entries[index].as_ref().unwrap().size_bytes;
            self.current_bytes = self.current_bytes.saturating_sub(old_size);
            if let Some(entry) = self.entries[index].as_mut() {
                entry.image = image;
                entry.size_bytes = size_bytes;
            }
            self.current_bytes += size_bytes;
            self.move_to_front(index);
            self.evict_if_needed();
            return;
        }

        // Evict until there is space
        while self.current_bytes + size_bytes > self.max_bytes && self.tail.is_some() {
            self.evict_tail();
        }

        // Allocate slot
        let index = if let Some(idx) = self.free_slots.pop() {
            self.entries[idx] = Some(CacheEntry {
                image,
                size_bytes,
                prev: None,
                next: self.head,
                path: path.clone(),
            });
            idx
        } else {
            let idx = self.entries.len();
            self.entries.push(Some(CacheEntry {
                image,
                size_bytes,
                prev: None,
                next: self.head,
                path: path.clone(),
            }));
            idx
        };

        if let Some(old_head) = self.head {
            if let Some(entry) = self.entries[old_head].as_mut() {
                entry.prev = Some(index);
            }
        } else {
            self.tail = Some(index);
        }
        self.head = Some(index);

        self.current_bytes += size_bytes;
        self.map.insert(path, index);
    }

    fn move_to_front(&mut self, index: usize) {
        if self.head == Some(index) {
            return; // Already at front
        }

        let (prev, next) = {
            let entry = self.entries[index].as_ref().unwrap();
            (entry.prev, entry.next)
        };

        // Unlink
        if let Some(p) = prev {
            self.entries[p].as_mut().unwrap().next = next;
        }
        if let Some(n) = next {
            self.entries[n].as_mut().unwrap().prev = prev;
        }
        if self.tail == Some(index) {
            self.tail = prev;
        }

        // Link at head
        let old_head = self.head;
        let entry = self.entries[index].as_mut().unwrap();
        entry.prev = None;
        entry.next = old_head;

        if let Some(h) = old_head {
            self.entries[h].as_mut().unwrap().prev = Some(index);
        }
        self.head = Some(index);
    }

    fn evict_tail(&mut self) {
        if let Some(tail_idx) = self.tail {
            let (prev, size, path) = {
                let entry = self.entries[tail_idx].as_ref().unwrap();
                (entry.prev, entry.size_bytes, entry.path.clone())
            };

            self.map.remove(&path);
            self.current_bytes = self.current_bytes.saturating_sub(size);

            if let Some(p) = prev {
                self.entries[p].as_mut().unwrap().next = None;
                self.tail = Some(p);
            } else {
                self.head = None;
                self.tail = None;
            }

            self.entries[tail_idx] = None;
            self.free_slots.push(tail_idx);
        }
    }

    fn evict_if_needed(&mut self) {
        while self.current_bytes > self.max_bytes && self.tail.is_some() {
            self.evict_tail();
        }
    }

    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

pub struct SharedCache {
    inner: Mutex<LruMemoryCache>,
}

impl SharedCache {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(LruMemoryCache::new(max_bytes)),
        }
    }

    pub fn get(&self, path: &Path) -> Option<Arc<DecodedImage>> {
        self.inner.lock().get(path)
    }

    pub fn insert(&self, path: PathBuf, image: Arc<DecodedImage>) {
        self.inner.lock().insert(path, image);
    }

    pub fn stats(&self) -> (usize, usize, usize, u64, u64) {
        let guard = self.inner.lock();
        (
            guard.len(),
            guard.current_bytes(),
            guard.max_bytes(),
            guard.hits,
            guard.misses,
        )
    }
}
