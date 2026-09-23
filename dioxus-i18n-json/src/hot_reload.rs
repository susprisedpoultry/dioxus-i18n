use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::time::Duration;

/// Watch `path` for changes and return a channel that yields `()` on every
/// detected file event.
pub fn watch_translations(
    path: impl AsRef<Path> + Send + 'static,
) -> tokio::sync::mpsc::Receiver<()> {
    let (tx, rx) = tokio::sync::mpsc::channel::<()>(1);

    // Spawn a blocking thread for the filesystem watcher so we don't block
    // the async runtime.
    tokio::task::spawn_blocking(move || {
        let (event_tx, event_rx) = std::sync::mpsc::channel::<Result<Event, notify::Error>>();

        let mut watcher = match RecommendedWatcher::new(event_tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to create file watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(path.as_ref(), RecursiveMode::NonRecursive) {
            tracing::error!("Failed to watch translations directory: {}", e);
            return;
        }

        loop {
            match event_rx.recv_timeout(Duration::from_millis(500)) {
                Ok(Ok(_event)) => {
                    // Debounce: wait a tiny bit for rapid-fire writes to settle.
                    std::thread::sleep(Duration::from_millis(100));
                    if tx.blocking_send(()).is_err() {
                        break;
                    }
                }
                Ok(Err(e)) => {
                    tracing::error!("Watch error: {}", e);
                }
                Err(_) => {
                    // Timeout – loop around so we periodically check for shutdown.
                }
            }
        }
    });

    rx
}
