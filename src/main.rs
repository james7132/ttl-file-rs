use notify::{RecommendedWatcher, RecursiveMode, Result, Watcher};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Component, Path};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

fn timestamp(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn parse_ttl(ttl: impl AsRef<OsStr>) -> Option<Duration> {
    humantime::parse_duration(ttl.as_ref().to_string_lossy().strip_prefix("ttl=")?).ok()
}

fn find_ttl(path: impl AsRef<Path>) -> Option<Duration> {
    for component in path.as_ref().components().rev() {
        if let Component::Normal(comp) = component {
            if let Some(ttl) = parse_ttl(comp) {
                return Some(ttl);
            }
        }
    }
    None
}

// Returns mapping of filepaths to expiration time
fn initialize_files(
    roots: impl IntoIterator<Item = impl AsRef<Path>>,
) -> HashMap<Box<Path>, SystemTime> {
    let mut files = HashMap::new();
    for result in roots.into_iter().flat_map(|root| WalkDir::new(root)) {
        match result {
            Ok(entry) => {
                let path = entry.path().canonicalize().unwrap();
                if entry.file_type().is_dir() {
                    log::info!("Watching directory {}", path.display());
                }
                if !entry.file_type().is_file() {
                    continue;
                }

                let ttl = if let Some(ttl) = find_ttl(&path) {
                    ttl
                } else {
                    log::info!("Skipping {} (no ttl in path)", path.display(),);
                    continue;
                };

                if let Ok(metadata) = entry.metadata() {
                    let creation_time = metadata.created().unwrap();
                    let expiration = creation_time + ttl;
                    log::info!(
                        "Found {} (ttl={}s) (expiration={})",
                        path.display(),
                        ttl.as_secs(),
                        timestamp(expiration),
                    );
                    files.insert(path.into_boxed_path(), expiration);
                }
            }
            Err(err) => {
                if let Some(path) = err.path() {
                    log::error!(
                        "Error while initializing watcher for {}: {}. Ignoring.",
                        path.display(),
                        err
                    );
                }
            }
        }
    }
    files
}

fn check_files(state: &mut HashMap<Box<Path>, SystemTime>) {
    let now = SystemTime::now();
    state.retain(|path, expiration| {
        if now <= *expiration {
            return true;
        }
        match std::fs::remove_file(path) {
            Ok(()) => {
                log::info!(
                    "Deleted {} ({} > {})",
                    path.display(),
                    timestamp(now),
                    timestamp(*expiration)
                );
                return false;
            }
            Err(err) => log::error!("Error while deleting {}: {}", path.display(), err),
        }
        true
    });
}

fn find_directories(dirs: impl Iterator<Item = String>) -> Vec<Box<Path>> {
    let mut directories = Vec::new();
    for input in dirs {
        match std::fs::canonicalize(&input) {
            Ok(path) => directories.push(path.into_boxed_path()),
            Err(err) => log::error!("Failed to watch {}. Skipping. Error: {}", input, err),
        }
    }
    directories
}

fn main() {
    env_logger::init();
    let mut directories = find_directories(std::env::args());
    if directories.is_empty() {
        directories.push(
            std::fs::canonicalize(std::env::current_dir().unwrap())
                .unwrap()
                .into_boxed_path(),
        );
    }
    let mut state = initialize_files(directories);

    loop {
        check_files(&mut state);
        std::thread::sleep(Duration::from_secs(1));
    }
}
