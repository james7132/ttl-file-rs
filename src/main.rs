#![forbid(unsafe_code)]

use dashmap::DashMap;
use notify::{
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
    Event, EventKind, RecursiveMode, Result, Watcher,
};
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

#[derive(Default)]
struct State {
    expirations: DashMap<PathBuf, SystemTime>,
}

impl State {
    fn add_file(&self, path: &PathBuf) {
        let ttl = if let Some(ttl) = find_ttl(path) {
            ttl
        } else {
            log::debug!("Skipping {} (no ttl in path)", path.display(),);
            return;
        };

        if let Ok(metadata) = std::fs::metadata(path) {
            let creation_time = metadata.created().unwrap();
            let expiration = creation_time + ttl;
            log::info!(
                "Watching file: {} (ttl={}s) (expiration={})",
                path.display(),
                ttl.as_secs(),
                timestamp(expiration),
            );
            self.expirations.insert(path.clone(), expiration);
        }
    }

    fn check_files(&self) {
        let now = SystemTime::now();
        self.expirations.retain(|path, expiration| {
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

    fn handle_notify_event(&self, event: Event) {
        match event {
            Event {
                kind: EventKind::Create(CreateKind::File),
                paths,
                ..
            } => {
                for path in paths.iter() {
                    log::info!("Watch file: {}", path.display());
                    self.add_file(path);
                }
            }
            Event {
                kind: EventKind::Remove(RemoveKind::File),
                paths,
                ..
            } => {
                for path in paths {
                    log::info!("Unwatch file: {}", path.display());
                    self.expirations.remove(&path);
                }
            }
            Event {
                kind: EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
                paths,
                ..
            } => {
                assert_eq!(paths.len(), 2);
                log::info!(
                    "Move file: {} -> {}",
                    paths[0].display(),
                    paths[1].display()
                );
                self.expirations.remove(&paths[0]);
                self.add_file(&paths[1]);
            }
            evt => log::debug!("Unknown Event: {:?}", evt),
        }
    }
}

fn timestamp(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn parse_ttl(ttl: &OsStr) -> Option<Duration> {
    humantime::parse_duration(ttl.to_string_lossy().strip_prefix("ttl=")?).ok()
}

fn find_ttl(path: &PathBuf) -> Option<Duration> {
    for component in path.components().rev() {
        if let Component::Normal(comp) = component {
            if let Some(ttl) = parse_ttl(comp) {
                return Some(ttl);
            }
        }
    }
    None
}

// Returns mapping of filepaths to expiration time
fn initialize_files(roots: impl IntoIterator<Item = impl AsRef<Path>>) -> State {
    let state = State::default();
    for result in roots.into_iter().flat_map(|root| WalkDir::new(root)) {
        match result {
            Ok(entry) => {
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path().canonicalize().unwrap();
                state.add_file(&path);
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
    state
}

fn find_directories(dirs: impl Iterator<Item = String>) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    for input in dirs {
        match std::fs::canonicalize(&input) {
            Ok(path) => directories.push(path),
            Err(err) => log::error!("Failed to watch {}. Skipping. Error: {}", input, err),
        }
    }
    directories
}

fn main() -> Result<()> {
    env_logger::init();
    let mut directories = find_directories(std::env::args());
    if directories.is_empty() {
        directories.push(std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap());
    }

    let state = Box::leak(Box::new(initialize_files(&directories)));
    let mut watcher = notify::recommended_watcher(|res| match res {
        Ok(event) => state.handle_notify_event(event),
        Err(e) => log::error!("watch error: {:?}", e),
    })?;

    for root in directories {
        watcher.watch(&root, RecursiveMode::Recursive)?;
    }

    loop {
        state.check_files();
        std::thread::sleep(Duration::from_secs(1));
    }
}
