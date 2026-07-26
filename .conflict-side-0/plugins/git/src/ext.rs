use std::path::Path;

use crate::operations::{local, merge, remote};
use crate::types::{CommitInfo, ConflictInfo, PullResult, PushResult, RemoteInfo, StatusInfo};

pub struct Git<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    _manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Git<'a, R, M> {
    pub fn is_repo(&self, path: &Path) -> bool {
        local::is_repo(path)
    }

    pub fn init(&self, path: &Path) -> Result<(), crate::Error> {
        local::init(path)
    }

    pub fn status(&self, path: &Path) -> Result<StatusInfo, crate::Error> {
        local::status(path)
    }

    pub fn add(&self, path: &Path, patterns: Vec<String>) -> Result<(), crate::Error> {
        local::add(path, patterns)
    }

    pub fn reset(&self, path: &Path, files: Vec<String>) -> Result<(), crate::Error> {
        local::reset(path, files)
    }

    pub fn commit(&self, path: &Path, message: &str) -> Result<String, crate::Error> {
        local::commit(path, message)
    }

    pub fn log(&self, path: &Path, limit: u32) -> Result<Vec<CommitInfo>, crate::Error> {
        local::log(path, limit)
    }

    pub fn get_current_branch(&self, path: &Path) -> Result<String, crate::Error> {
        local::get_current_branch(path)
    }

    pub fn add_remote(&self, path: &Path, name: &str, url: &str) -> Result<(), crate::Error> {
        remote::add_remote(path, name, url)
    }

    pub fn list_remotes(&self, path: &Path) -> Result<Vec<RemoteInfo>, crate::Error> {
        remote::list_remotes(path)
    }

    pub fn fetch(&self, path: &Path, remote_name: &str) -> Result<(), crate::Error> {
        remote::fetch(path, remote_name)
    }

    pub fn push(
        &self,
        path: &Path,
        remote_name: &str,
        branch: &str,
    ) -> Result<PushResult, crate::Error> {
        remote::push(path, remote_name, branch)
    }

    pub fn pull(
        &self,
        path: &Path,
        remote_name: &str,
        branch: &str,
    ) -> Result<PullResult, crate::Error> {
        remote::pull(path, remote_name, branch)
    }

    pub fn check_conflicts(&self, path: &Path) -> Result<Option<ConflictInfo>, crate::Error> {
        merge::check_conflicts(path)
    }

    pub fn abort_merge(&self, path: &Path) -> Result<(), crate::Error> {
        merge::abort_merge(path)
    }
}

pub trait GitPluginExt<R: tauri::Runtime> {
    fn git(&self) -> Git<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> GitPluginExt<R> for T {
    fn git(&self) -> Git<'_, R, Self>
    where
        Self: Sized,
    {
        Git {
            _manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
