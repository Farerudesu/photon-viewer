use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::thread;

pub struct FolderScanner;

impl FolderScanner {
    pub fn scan_async(
        current_file: PathBuf,
        cancellation: Arc<AtomicBool>,
        on_complete: crossbeam_channel::Sender<(Vec<PathBuf>, usize)>,
    ) {
        thread::spawn(move || {
            let parent = match current_file.parent() {
                Some(p) => p.to_path_buf(),
                None => return,
            };

            let read_dir = match fs::read_dir(&parent) {
                Ok(rd) => rd,
                Err(_) => return,
            };

            let mut files = Vec::with_capacity(128);

            for entry in read_dir.flatten() {
                if cancellation.load(AtomicOrdering::Relaxed) {
                    return;
                }

                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        let path = entry.path();
                        // Filter by supported extensions as a fast heuristic
                        if is_potential_image_file(&path) {
                            files.push(path);
                        }
                    }
                }
            }

            // Natural sort
            files.sort_by(|a, b| natural_sort_cmp(a, b));

            // Find current file index
            let current_index = files
                .iter()
                .position(|p| p == &current_file)
                .unwrap_or(0);

            let _ = on_complete.send((files, current_index));
        });
    }
}

pub fn is_potential_image_file(path: &Path) -> bool {
    let Some(ext) = path.extension() else {
        return false; // Extensionless files (e.g. LICENSE, Makefile) are NOT images
    };

    const SUPPORTED_EXTS: &[&str] = &[
        "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif",
        "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2",
        "raf", "orf", "rw2", "dng", "pef", "srw", "3fr", "fff",
        "iiq", "x3f", "rwl", "gpr", "kdc", "dcr", "mrw", "erf",
        "mef", "hdr", "exr", "psd",
        "ico", "tga", "svg", "pnm", "pam", "ppm", "pgm", "dds", "qoi",
    ];

    SUPPORTED_EXTS.iter().any(|&s| ext.eq_ignore_ascii_case(s))
}

/// Natural sort comparison: compares numeric segments numerically, letters alphabetically
pub fn natural_sort_cmp(a: &Path, b: &Path) -> Ordering {
    let a_str = a.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    let b_str = b.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();

    let mut a_chars = a_str.chars().peekable();
    let mut b_chars = b_str.chars().peekable();

    while let (Some(&ac), Some(&bc)) = (a_chars.peek(), b_chars.peek()) {
        if ac.is_ascii_digit() && bc.is_ascii_digit() {
            // Compare number chunks
            let mut a_num: u64 = 0;
            while let Some(&c) = a_chars.peek() {
                if let Some(digit) = c.to_digit(10) {
                    a_num = a_num.saturating_mul(10).saturating_add(digit as u64);
                    a_chars.next();
                } else {
                    break;
                }
            }

            let mut b_num: u64 = 0;
            while let Some(&c) = b_chars.peek() {
                if let Some(digit) = c.to_digit(10) {
                    b_num = b_num.saturating_mul(10).saturating_add(digit as u64);
                    b_chars.next();
                } else {
                    break;
                }
            }

            match a_num.cmp(&b_num) {
                Ordering::Equal => continue,
                other => return other,
            }
        } else {
            let ac_lower = ac.to_ascii_lowercase();
            let bc_lower = bc.to_ascii_lowercase();
            match ac_lower.cmp(&bc_lower) {
                Ordering::Equal => {
                    a_chars.next();
                    b_chars.next();
                }
                other => return other,
            }
        }
    }

    a_str.len().cmp(&b_str.len())
}
