use std::path::Path;

use crate::types::{PullResult, PushResult, RemoteInfo};

pub fn add_remote(path: &Path, name: &str, url: &str) -> Result<(), crate::Error> {
    let repo = gix::discover(path)?;
    let config_path = repo.git_dir().join("config");

    let mut config_content = if config_path.exists() {
        std::fs::read_to_string(&config_path)?
    } else {
        String::new()
    };

    let remote_section = format!(
        "\n[remote \"{}\"]\n\turl = {}\n\tfetch = +refs/heads/*:refs/remotes/{}/*\n",
        name, url, name
    );

    let check_str = format!("[remote \"{}\"]", name);
    if !config_content.contains(&check_str) {
        config_content.push_str(&remote_section);
        std::fs::write(&config_path, config_content)?;
    }

    Ok(())
}

pub fn list_remotes(path: &Path) -> Result<Vec<RemoteInfo>, crate::Error> {
    let repo = gix::discover(path)?;
    let mut remotes = Vec::new();

    for remote_name in repo.remote_names() {
        if let Ok(remote) = repo.find_remote(remote_name.as_ref())
            && let Some(url) = remote.url(gix::remote::Direction::Fetch)
        {
            remotes.push(RemoteInfo {
                name: remote_name.to_string(),
                url: url.to_bstring().to_string(),
            });
        }
    }

    Ok(remotes)
}

pub fn fetch(path: &Path, remote_name: &str) -> Result<(), crate::Error> {
    let repo = gix::discover(path)?;

    let remote = repo
        .find_remote(remote_name)
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    let connection = remote
        .connect(gix::remote::Direction::Fetch)
        .map_err(|e| crate::Error::Custom(format!("Failed to connect: {}", e)))?;

    connection
        .prepare_fetch(gix::progress::Discard, Default::default())
        .map_err(|e| crate::Error::Custom(format!("Failed to prepare fetch: {}", e)))?
        .receive(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
        .map_err(|e| crate::Error::Custom(format!("Failed to fetch: {}", e)))?;

    Ok(())
}

pub fn push(path: &Path, remote_name: &str, _branch: &str) -> Result<PushResult, crate::Error> {
    let repo = gix::discover(path)?;

    let remote = repo
        .find_remote(remote_name)
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    let _connection = remote
        .connect(gix::remote::Direction::Push)
        .map_err(|e| crate::Error::Custom(format!("Failed to connect: {}", e)))?;

    Ok(PushResult::Rejected {
        reason: "Push not yet implemented for gix 0.72".to_string(),
    })
}

pub fn pull(path: &Path, remote_name: &str, branch: &str) -> Result<PullResult, crate::Error> {
    fetch(path, remote_name)?;

    let repo = gix::discover(path)?;

    let remote_ref = format!("refs/remotes/{}/{}", remote_name, branch);
    let remote_commit = match repo.find_reference(&remote_ref) {
        Ok(mut reference) => reference
            .peel_to_id_in_place()
            .map_err(|e| crate::Error::Custom(e.to_string()))?
            .detach(),
        Err(_) => return Ok(PullResult::AlreadyUpToDate),
    };

    let local_ref = format!("refs/heads/{}", branch);
    let local_commit = match repo.find_reference(&local_ref) {
        Ok(mut reference) => reference
            .peel_to_id_in_place()
            .map_err(|e| crate::Error::Custom(e.to_string()))?
            .detach(),
        Err(_) => {
            let head_ref = repo.git_dir().join("refs/heads").join(branch);
            if let Some(parent) = head_ref.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&head_ref, format!("{}\n", remote_commit))?;
            return Ok(PullResult::Success { commits_pulled: 1 });
        }
    };

    if local_commit == remote_commit {
        return Ok(PullResult::AlreadyUpToDate);
    }

    let head_ref = repo.git_dir().join("refs/heads").join(branch);
    std::fs::write(&head_ref, format!("{}\n", remote_commit))?;

    let remote_tree = repo
        .find_object(remote_commit)
        .map_err(|e| crate::Error::Custom(e.to_string()))?
        .try_into_commit()
        .map_err(|e| crate::Error::Custom(e.to_string()))?
        .tree_id()
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    let workdir = repo
        .workdir()
        .ok_or_else(|| crate::Error::Custom("No working directory".to_string()))?;

    let tree_obj = repo
        .find_object(remote_tree)
        .map_err(|e| crate::Error::Custom(e.to_string()))?
        .try_into_tree()
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    let entries: Result<Vec<_>, _> = tree_obj.iter().collect();
    let entries = entries.map_err(|e| crate::Error::Custom(e.to_string()))?;

    for entry in entries {
        super::merge::restore_tree_entry(&repo, workdir, entry.inner.into(), Vec::new())?;
    }

    let mut new_state = gix::index::State::new(repo.object_hash());
    super::merge::populate_index_from_tree(&repo, &mut new_state, remote_tree.into(), Vec::new())?;

    let index_path = repo.git_dir().join("index");
    let new_index = gix::index::File::from_state(new_state, index_path.clone());
    let options = gix::index::write::Options::default();
    let file = std::fs::File::create(&index_path)?;
    new_index
        .write_to(file, options)
        .map_err(|e| crate::Error::Custom(e.to_string()))?;

    Ok(PullResult::Success { commits_pulled: 1 })
}
