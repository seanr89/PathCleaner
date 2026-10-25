use std::path::Path;
use std::time::SystemTime;

use jwalk::WalkDir;

#[derive(Debug, Clone, Copy)]
pub struct DirectoryStats {
    pub size_bytes: u64,
    pub file_count: u64,
    pub last_modified: Option<SystemTime>,
}

/// Computes total logical byte size, file count, and latest modification time for a directory tree.
pub fn compute_directory_stats(path: &Path) -> DirectoryStats {
    let mut size_bytes = 0u64;
    let mut file_count = 0u64;
    let mut latest_mtime: Option<SystemTime> = path.metadata().and_then(|m| m.modified()).ok();

    for entry in WalkDir::new(path)
        .skip_hidden(false)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                size_bytes = size_bytes.saturating_add(meta.len());
                file_count = file_count.saturating_add(1);
            }
            if let Ok(mtime) = meta.modified() {
                latest_mtime = Some(match latest_mtime {
                    Some(prev) if prev >= mtime => prev,
                    _ => mtime,
                });
            }
        }
    }

    DirectoryStats {
        size_bytes,
        file_count,
        last_modified: latest_mtime,
    }
}
