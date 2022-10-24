use notify::{
    event::{CreateKind, ModifyKind, RemoveKind},
    Event, EventKind, RecursiveMode, Result, Watcher,
};
use std::ffi::OsStr;
use std::path::{Component, Path};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

#[derive(Default)]
struct State {
    expirations: std::collections::HashMap<Box<Path>, SystemTime>,
}

impl State {
    fn add_file(&mut self, path: Box<Path>) {
        let ttl = if let Some(ttl) = find_ttl(&path) {
            ttl
        } else {
            log::info!("Skipping {} (no ttl in path)", path.display(),);
            return;
        };

        if let Ok(metadata) = std::fs::metadata(&path) {
            let creation_time = metadata.created().unwrap();
            let expiration = creation_time + ttl;
            log::info!(
                "Found {} (ttl={}s) (expiration={})",
                path.display(),
                ttl.as_secs(),
                timestamp(expiration),
            );
            self.expirations.insert(path.clone(), expiration);
        }
    }

    fn check_files(&mut self) {
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

    fn handle_notify_event(&mut self, event: Event) {
        match event {
            // Created new files
            Event {
                kind: EventKind::Create(info),
                paths,
                ..
            } => {
                if info == CreateKind::File {
                    for path in paths {
                        self.add_file(path.into_boxed_path());
                    }
                }
            }
            Event {
                kind: EventKind::Remove(RemoveKind::File),
                paths,
                ..
            } => {
                for path in paths {
                    self.expirations.remove(
                        &path.canonicalize().unwrap().into_boxed_path()
                    );
                }
            }
            Event {
                kind: EventKind::Remove(RemoveKind::Folder),
                paths,
                ..
            } => {
                for path in paths {
                    let path = path.canonicalize().unwrap();
                    self.expirations.retain(|file_path, _| {
                        let file_path = file_path.canonicalize().unwrap();
                        !file_path.starts_with(&path)
                    });
                }
            }
            Event {
                kind: EventKind::Modify(ModifyKind::Name(_)),
                ..
            } => {
                // TODO(james7132): Complete this implementation
            }
            _ => {}
        }
    }
}

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
fn initialize_files(roots: impl IntoIterator<Item = impl AsRef<Path>>) -> State {
    let mut state = State::default();
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
                state.add_file(path.into_boxed_path());
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

fn main() -> Result<()> {
    env_logger::init();
    let mut directories = find_directories(std::env::args());
    if directories.is_empty() {
        directories.push(
            std::fs::canonicalize(std::env::current_dir().unwrap())
                .unwrap()
                .into_boxed_path(),
        );
    }

    let state = Arc::new(Mutex::new(initialize_files(&directories)));
    let notify_state = state.clone();
    let mut watcher = notify::recommended_watcher(move |res| match res {
        Ok(event) => notify_state.lock().unwrap().handle_notify_event(event),
        Err(e) => println!("watch error: {:?}", e),
    })?;

    for root in directories {
        watcher.watch(&root, RecursiveMode::Recursive)?;
    }

    loop {
        state.lock().unwrap().check_files();
        std::thread::sleep(Duration::from_secs(1));
    }
}
