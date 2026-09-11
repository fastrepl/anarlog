//! The Tauri app's `startup.rs` channel lock: Nightly and stable open the same
//! database, and SQLite serializes their writes, but live queries only see
//! their own process, so the two channels exclude each other for the whole
//! app lifetime. Each channel holds its own lock file in the database folder;
//! the peer's lock being held means the peer is running. A held own lock is
//! left to the single-instance socket, as in the Tauri app.

use std::fs::TryLockError;
use std::path::Path;

pub struct ChannelLock {
    _file: std::fs::File,
}

pub enum ChannelLockState {
    Acquired(Option<ChannelLock>),
    PeerRunning { peer: &'static str },
    Unavailable(String),
}

enum LockState {
    Acquired(std::fs::File),
    HeldByAnotherProcess,
    Unavailable(String),
}

fn lock_file(path: &Path) -> LockState {
    let file = match std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) => {
            return LockState::Unavailable(format!("failed to open {}: {error}", path.display()));
        }
    };
    match file.try_lock() {
        Ok(()) => LockState::Acquired(file),
        Err(TryLockError::WouldBlock) => LockState::HeldByAnotherProcess,
        Err(TryLockError::Error(error)) => {
            LockState::Unavailable(format!("failed to lock {}: {error}", path.display()))
        }
    }
}

/// `acquire_channel_lock`: nothing to hold for channels without a database
/// peer.
pub fn acquire_channel_lock(identifier: &str, db_dir: &Path) -> ChannelLockState {
    let Some(peer) = crate::db::shared_database_peer(identifier) else {
        return ChannelLockState::Acquired(None);
    };
    lock_channel_files(
        &db_dir.join(channel_lock_filename(identifier)),
        &db_dir.join(channel_lock_filename(peer)),
        peer,
    )
}

fn channel_lock_filename(identifier: &str) -> String {
    format!("{identifier}.running.lock")
}

// The peer is probed before our own lock is taken. Two channels starting at
// the same instant can both slip through, which only preserves today's
// behavior; the reverse order could make both of them exit.
fn lock_channel_files(own_path: &Path, peer_path: &Path, peer: &'static str) -> ChannelLockState {
    match lock_file(peer_path) {
        LockState::Acquired(probe) => drop(probe),
        LockState::HeldByAnotherProcess => return ChannelLockState::PeerRunning { peer },
        LockState::Unavailable(reason) => return ChannelLockState::Unavailable(reason),
    }
    match lock_file(own_path) {
        LockState::Acquired(file) => ChannelLockState::Acquired(Some(ChannelLock { _file: file })),
        LockState::HeldByAnotherProcess => ChannelLockState::Acquired(None),
        LockState::Unavailable(reason) => ChannelLockState::Unavailable(reason),
    }
}

/// `exit_for_running_peer_channel`: the message the Tauri app prints (its
/// macOS alert has no Linux counterpart).
pub fn exit_for_running_peer_channel(identifier: &str, peer: &str) -> ! {
    let own_name = channel_product_name(identifier);
    let peer_name = channel_product_name(peer);
    eprintln!("{peer_name} is running on the shared database; exiting {own_name}");
    std::process::exit(0);
}

fn channel_product_name(identifier: &str) -> &'static str {
    if identifier == crate::db::NIGHTLY_BUNDLE_ID {
        "Anarlog Nightly"
    } else {
        "Anarlog"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_lock_excludes_the_peer_channel_while_held() {
        let dir = tempfile::tempdir().unwrap();
        let stable = dir
            .path()
            .join(channel_lock_filename("com.hyprnote.stable"));
        let nightly = dir
            .path()
            .join(channel_lock_filename("com.hyprnote.nightly"));

        let stable_lock = lock_channel_files(&stable, &nightly, "com.hyprnote.nightly");
        assert!(matches!(stable_lock, ChannelLockState::Acquired(Some(_))));
        assert!(matches!(
            lock_channel_files(&nightly, &stable, "com.hyprnote.stable"),
            ChannelLockState::PeerRunning {
                peer: "com.hyprnote.stable"
            }
        ));
        drop(stable_lock);
        assert!(matches!(
            lock_channel_files(&nightly, &stable, "com.hyprnote.stable"),
            ChannelLockState::Acquired(Some(_))
        ));
    }

    #[test]
    fn channels_without_a_peer_hold_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            acquire_channel_lock("com.hyprnote.dev", dir.path()),
            ChannelLockState::Acquired(None)
        ));
        assert_eq!(
            channel_product_name("com.hyprnote.nightly"),
            "Anarlog Nightly"
        );
        assert_eq!(channel_product_name("com.hyprnote.stable"), "Anarlog");
    }
}
