use std::path::Path;

use crate::types::{CommitInfo, FileChangeType, FileStatus, StatusInfo};

const DEFAULT_GITIGNORE: &str = "# Char auto-generated gitignore
# Large audio files (not suitable for git)
*.wav
*.ogg
*.mp3
*.m4a
*.flac
sessions/*/audio*.wav
sessions/*/audio*.ogg
";

pub fn is_repo(path: &Path) -> bool {
    gix::discover(path).is_ok()
}

pub fn init(path: &Path) -> Result<(), crate::Error> {
    gix::init(path)?;

    let gitignore_path = path.join(".gitignore");
    if !gitignore_path.exists() {
        std::fs::write(gitignore_path, DEFAULT_GITIGNORE)?;
    }

    Ok(())
}

pub fn status(path: &Path) -> Result<StatusInfo, crate::Error> {
    let repo = gix::discover(path)?;

    let mut status_info = StatusInfo {
        staged: Vec::new(),
        unstaged: Vec::new(),
        untracked: Vec::new(),
        conflicted: Vec::new(),
        has_changes: false,
    };

    let index = repo
        .index_or_empty()
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    let workdir = repo
        .workdir()
        .ok_or_else(|| crate::Error::Custom("No working directory".to_string()))?;

    for entry in walkdir::WalkDir::new(workdir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with(".git")
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let rel_path = entry
                .path()
                .strip_prefix(workdir)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .to_string();

            let in_index = index
                .entries()
                .iter()
                .any(|e| String::from_utf8_lossy(e.path(&index)) == rel_path);

            if !in_index {
                status_info.untracked.push(rel_path);
                status_info.has_changes = true;
            }
        }
    }

    for entry in index.entries() {
        let entry_path = String::from_utf8_lossy(entry.path(&index)).to_string();
        let full_path = workdir.join(&entry_path);

        if !full_path.exists() {
            status_info.unstaged.push(FileStatus {
                path: entry_path,
                status: FileChangeType::Deleted,
            });
            status_info.has_changes = true;
        } else if let Ok(metadata) = std::fs::metadata(&full_path) {
            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let index_mtime = entry.stat.mtime.secs as i64;

            if mtime != index_mtime
                && let Ok(current_data) = std::fs::read(&full_path)
                && let Ok(current_hash) = gix::objs::compute_hash(
                    repo.object_hash(),
                    gix::objs::Kind::Blob,
                    &current_data,
                )
                && current_hash != entry.id
            {
                status_info.unstaged.push(FileStatus {
                    path: entry_path,
                    status: FileChangeType::Modified,
                });
                status_info.has_changes = true;
            }
        }
    }

    Ok(status_info)
}

pub fn add(path: &Path, patterns: Vec<String>) -> Result<(), crate::Error> {
    let repo = gix::discover(path)?;
    let index_path = repo.git_dir().join("index");
    let workdir = repo
        .workdir()
        .ok_or_else(|| crate::Error::Custom("No working directory".to_string()))?;

    let index = if index_path.exists() {
        gix::index::File::at(
            &index_path,
            repo.object_hash(),
            false,
            gix::index::decode::Options::default(),
        )
        .map_err(|e| crate::Error::Custom(e.to_string()))?
    } else {
        gix::index::File::from_state(
            gix::index::State::new(repo.object_hash()),
            index_path.clone(),
        )
    };

    let mut entries_to_add: Vec<(String, gix::ObjectId, gix::index::entry::Stat)> = Vec::new();

    for pattern in patterns {
        if pattern == "." || pattern == "*" {
            for entry in walkdir::WalkDir::new(workdir)
                .into_iter()
                .filter_entry(|e| {
                    let name = e.file_name().to_string_lossy();
                    !name.starts_with(".git")
                })
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    let rel_path = entry
                        .path()
                        .strip_prefix(workdir)
                        .unwrap_or(entry.path())
                        .to_string_lossy()
                        .to_string();

                    if let Ok(data) = std::fs::read(entry.path()) {
                        let oid = repo
                            .write_blob(&data)
                            .map_err(|e| crate::Error::Custom(e.to_string()))?;

                        let metadata = std::fs::metadata(entry.path())?;
                        let stat = create_stat_from_metadata(&metadata);

                        entries_to_add.push((rel_path, oid.into(), stat));
                    }
                }
            }
        } else {
            let file_path = workdir.join(&pattern);
            if file_path.exists() && file_path.is_file() {
                let data = std::fs::read(&file_path)?;
                let oid = repo
                    .write_blob(&data)
                    .map_err(|e| crate::Error::Custom(e.to_string()))?;

                let metadata = std::fs::metadata(&file_path)?;
                let stat = create_stat_from_metadata(&metadata);

                entries_to_add.push((pattern, oid.into(), stat));
            }
        }
    }

    let mut new_state = gix::index::State::new(repo.object_hash());

    for entry in index.entries() {
        let path = String::from_utf8_lossy(entry.path(&index)).to_string();
        if !entries_to_add.iter().any(|(p, _, _)| p == &path) {
            new_state.dangerously_push_entry(
                entry.stat,
                entry.id,
                entry.flags,
                entry.mode,
                entry.path(&index),
            );
        }
    }

    for (rel_path, oid, stat) in entries_to_add {
        new_state.dangerously_push_entry(
            stat,
            oid,
            gix::index::entry::Flags::empty(),
            gix::index::entry::Mode::FILE,
            rel_path.as_bytes().into(),
        );
    }

    let new_index = gix::index::File::from_state(new_state, index_path.clone());

    let options = gix::index::write::Options::default();
    let file = std::fs::File::create(&index_path)?;
    new_index
        .write_to(file, options)
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    Ok(())
}

pub fn reset(path: &Path, files: Vec<String>) -> Result<(), crate::Error> {
    let repo = gix::discover(path)?;
    let index_path = repo.git_dir().join("index");

    if !index_path.exists() {
        return Ok(());
    }

    let index = gix::index::File::at(
        &index_path,
        repo.object_hash(),
        false,
        gix::index::decode::Options::default(),
    )
    .map_err(|e| crate::Error::Custom(e.to_string()))?;

    let files_set: std::collections::HashSet<&str> = files.iter().map(|s| s.as_str()).collect();

    let mut new_state = gix::index::State::new(repo.object_hash());

    for entry in index.entries() {
        let entry_path = String::from_utf8_lossy(entry.path(&index));
        if files.is_empty() || !files_set.contains(entry_path.as_ref()) {
            new_state.dangerously_push_entry(
                entry.stat,
                entry.id,
                entry.flags,
                entry.mode,
                entry.path(&index),
            );
        }
    }

    let new_index = gix::index::File::from_state(new_state, index_path.clone());

    let options = gix::index::write::Options::default();
    let file = std::fs::File::create(&index_path)?;
    new_index
        .write_to(file, options)
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    Ok(())
}

pub fn commit(path: &Path, message: &str) -> Result<String, crate::Error> {
    let repo = gix::discover(path)?;

    let tree_id = {
        let index = repo
            .index_or_empty()
            .map_err(|e| crate::Error::Custom(e.to_string()))?;

        let mut trees: std::collections::HashMap<Vec<u8>, gix::objs::Tree> =
            std::collections::HashMap::new();

        for entry in index.entries() {
            let path_bytes = entry.path(&index);
            let path_str =
                std::str::from_utf8(path_bytes).map_err(|e| crate::Error::Custom(e.to_string()))?;
            let parts: Vec<&str> = path_str.split('/').collect();

            if parts.len() == 1 {
                trees
                    .entry(Vec::new())
                    .or_insert_with(gix::objs::Tree::empty)
                    .entries
                    .push(gix::objs::tree::Entry {
                        mode: gix::objs::tree::EntryKind::Blob.into(),
                        filename: parts[0].as_bytes().into(),
                        oid: entry.id,
                    });
            } else {
                for i in 0..parts.len() {
                    let parent_path = if i == 0 {
                        Vec::new()
                    } else {
                        parts[..i].join("/").into_bytes()
                    };

                    if i == parts.len() - 1 {
                        trees
                            .entry(parent_path)
                            .or_insert_with(gix::objs::Tree::empty)
                            .entries
                            .push(gix::objs::tree::Entry {
                                mode: gix::objs::tree::EntryKind::Blob.into(),
                                filename: parts[i].as_bytes().into(),
                                oid: entry.id,
                            });
                    } else {
                        trees
                            .entry(parent_path)
                            .or_insert_with(gix::objs::Tree::empty);
                    }
                }
            }
        }

        let mut written_trees: std::collections::HashMap<Vec<u8>, gix::ObjectId> =
            std::collections::HashMap::new();
        let mut sorted_paths: Vec<Vec<u8>> = trees.keys().cloned().collect();
        sorted_paths.sort_by_key(|b| std::cmp::Reverse(b.len()));

        for tree_path in sorted_paths {
            let mut tree = trees.remove(&tree_path).unwrap();

            for entry in &mut tree.entries {
                let child_path = if tree_path.is_empty() {
                    std::str::from_utf8(&entry.filename)
                        .unwrap()
                        .as_bytes()
                        .to_vec()
                } else {
                    [&tree_path[..], b"/", &entry.filename[..]].concat()
                };

                if let Some(&child_tree_id) = written_trees.get(&child_path) {
                    entry.mode = gix::objs::tree::EntryKind::Tree.into();
                    entry.oid = child_tree_id;
                }
            }

            tree.entries.sort_by(|a, b| a.filename.cmp(&b.filename));

            let tree_id = repo
                .write_object(&tree)
                .map_err(|e| crate::Error::Custom(e.to_string()))?;
            written_trees.insert(tree_path, tree_id.into());
        }

        written_trees
            .get(&Vec::new())
            .copied()
            .ok_or_else(|| crate::Error::Custom("Failed to create root tree".to_string()))?
    };

    let parents: Vec<gix::ObjectId> = repo
        .head_id()
        .ok()
        .map(|id| id.detach())
        .into_iter()
        .collect();

    let commit_id = repo
        .commit("HEAD", message, tree_id, parents)
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    Ok(commit_id.to_string())
}

pub fn log(path: &Path, limit: u32) -> Result<Vec<CommitInfo>, crate::Error> {
    let repo = gix::discover(path)?;
    let mut commits = Vec::new();

    let head = match repo.head_id() {
        Ok(id) => id,
        Err(_) => return Ok(commits),
    };

    let mut current = Some(head.detach());
    let mut count = 0;

    while let Some(oid) = current {
        if count >= limit {
            break;
        }

        let commit = repo
            .find_object(oid)
            .map_err(|e| crate::Error::Custom(e.to_string()))?
            .try_into_commit()
            .map_err(|e| crate::Error::Custom(e.to_string()))?;

        let commit_ref = commit
            .decode()
            .map_err(|e| crate::Error::Custom(e.to_string()))?;

        commits.push(CommitInfo {
            id: oid.to_string(),
            message: commit_ref.message.to_string(),
            author: commit_ref.author.name.to_string(),
            timestamp: commit_ref
                .author
                .time
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
        });

        current = commit_ref.parents().next();
        count += 1;
    }

    Ok(commits)
}

pub fn get_current_branch(path: &Path) -> Result<String, crate::Error> {
    let repo = gix::discover(path)?;

    let head = repo
        .head()
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    match head.referent_name() {
        Some(name) => {
            let full_name = name.as_bstr().to_string();
            let branch = full_name
                .strip_prefix("refs/heads/")
                .unwrap_or(&full_name)
                .to_string();
            Ok(branch)
        }
        None => Ok("HEAD".to_string()),
    }
}

pub(super) fn create_stat_from_metadata(metadata: &std::fs::Metadata) -> gix::index::entry::Stat {
    use std::time::UNIX_EPOCH;

    let mtime = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);

    let ctime = metadata
        .created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as u32)
        .unwrap_or(mtime);

    let size = metadata.len() as u32;

    gix::index::entry::Stat {
        mtime: gix::index::entry::stat::Time {
            secs: mtime,
            nsecs: 0,
        },
        ctime: gix::index::entry::stat::Time {
            secs: ctime,
            nsecs: 0,
        },
        dev: 0,
        ino: 0,
        uid: 0,
        gid: 0,
        size,
    }
}
