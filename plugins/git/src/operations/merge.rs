use std::path::Path;

use crate::types::ConflictInfo;

pub fn check_conflicts(path: &Path) -> Result<Option<ConflictInfo>, crate::Error> {
    let repo = gix::discover(path)?;

    let merge_head = repo.git_dir().join("MERGE_HEAD");
    if !merge_head.exists() {
        return Ok(None);
    }

    let index = repo
        .index_or_empty()
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    let mut conflicted_files = Vec::new();

    for entry in index.entries() {
        if entry.stage() != gix::index::entry::Stage::Unconflicted {
            let path = String::from_utf8_lossy(entry.path(&index)).to_string();
            if !conflicted_files.contains(&path) {
                conflicted_files.push(path);
            }
        }
    }

    if conflicted_files.is_empty() {
        Ok(None)
    } else {
        Ok(Some(ConflictInfo {
            files: conflicted_files,
        }))
    }
}

pub fn abort_merge(path: &Path) -> Result<(), crate::Error> {
    let repo = gix::discover(path)?;
    let git_dir = repo.git_dir();

    let merge_head = git_dir.join("MERGE_HEAD");
    let merge_msg = git_dir.join("MERGE_MSG");
    let merge_mode = git_dir.join("MERGE_MODE");

    if merge_head.exists() {
        std::fs::remove_file(merge_head)?;
    }
    if merge_msg.exists() {
        std::fs::remove_file(merge_msg)?;
    }
    if merge_mode.exists() {
        std::fs::remove_file(merge_mode)?;
    }

    let head_commit = repo
        .head_id()
        .map_err(|e| crate::Error::Custom(e.to_string()))?
        .detach();

    let head_tree = repo
        .find_object(head_commit)
        .map_err(|e| crate::Error::Custom(e.to_string()))?
        .try_into_commit()
        .map_err(|e| crate::Error::Custom(e.to_string()))?
        .tree_id()
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    let workdir = repo
        .workdir()
        .ok_or_else(|| crate::Error::Custom("No working directory".to_string()))?;

    let tree_obj = repo
        .find_object(head_tree)
        .map_err(|e| crate::Error::Custom(e.to_string()))?
        .try_into_tree()
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    let entries: Result<Vec<_>, _> = tree_obj.iter().collect();
    let entries = entries.map_err(|e| crate::Error::Custom(e.to_string()))?;

    for entry in entries {
        restore_tree_entry(&repo, workdir, entry.inner.into(), Vec::new())?;
    }

    let mut new_state = gix::index::State::new(repo.object_hash());
    populate_index_from_tree(&repo, &mut new_state, head_tree.into(), Vec::new())?;

    let index_path = git_dir.join("index");
    let new_index = gix::index::File::from_state(new_state, index_path.clone());
    let options = gix::index::write::Options::default();
    let file = std::fs::File::create(&index_path)?;
    new_index
        .write_to(file, options)
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    Ok(())
}

pub(super) fn restore_tree_entry(
    repo: &gix::Repository,
    workdir: &Path,
    entry: gix::objs::tree::Entry,
    parent_path: Vec<u8>,
) -> Result<(), crate::Error> {
    let entry_path = if parent_path.is_empty() {
        entry.filename.to_vec()
    } else {
        [&parent_path[..], b"/", &entry.filename[..]].concat()
    };

    let file_path = workdir.join(std::str::from_utf8(&entry_path).unwrap());

    if entry.mode.is_tree() {
        if !file_path.exists() {
            std::fs::create_dir_all(&file_path)?;
        }

        let tree_obj = repo
            .find_object(entry.oid)
            .map_err(|e| crate::Error::Custom(e.to_string()))?
            .try_into_tree()
            .map_err(|e| crate::Error::Custom(e.to_string()))?;

        let child_entries: Result<Vec<_>, _> = tree_obj.iter().collect();
        let child_entries = child_entries.map_err(|e| crate::Error::Custom(e.to_string()))?;

        for child_entry in child_entries {
            restore_tree_entry(repo, workdir, child_entry.inner.into(), entry_path.clone())?;
        }
    } else {
        let blob = repo
            .find_object(entry.oid)
            .map_err(|e| crate::Error::Custom(e.to_string()))?
            .try_into_blob()
            .map_err(|e| crate::Error::Custom(e.to_string()))?;

        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, &blob.data)?;
    }

    Ok(())
}

pub(super) fn populate_index_from_tree(
    repo: &gix::Repository,
    state: &mut gix::index::State,
    tree_id: gix::ObjectId,
    parent_path: Vec<u8>,
) -> Result<(), crate::Error> {
    let tree_obj = repo
        .find_object(tree_id)
        .map_err(|e| crate::Error::Custom(e.to_string()))?
        .try_into_tree()
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    let workdir = repo
        .workdir()
        .ok_or_else(|| crate::Error::Custom("No working directory".to_string()))?;

    let entries: Result<Vec<_>, _> = tree_obj.iter().collect();
    let entries = entries.map_err(|e| crate::Error::Custom(e.to_string()))?;

    for entry in entries {
        let entry_path = if parent_path.is_empty() {
            entry.inner.filename.to_vec()
        } else {
            [&parent_path[..], b"/", entry.inner.filename].concat()
        };

        if entry.inner.mode.is_tree() {
            populate_index_from_tree(repo, state, entry.inner.oid.into(), entry_path)?;
        } else {
            let file_path = workdir.join(std::str::from_utf8(&entry_path).unwrap());
            let metadata = std::fs::metadata(&file_path)?;
            let stat = super::local::create_stat_from_metadata(&metadata);

            state.dangerously_push_entry(
                stat,
                entry.inner.oid.into(),
                gix::index::entry::Flags::empty(),
                gix::index::entry::Mode::FILE,
                entry_path.as_slice().into(),
            );
        }
    }

    Ok(())
}
