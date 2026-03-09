#![cfg(test)]

use std::path::{Path, PathBuf};

use assert_fs::TempDir;
use assert_fs::fixture::ChildPath;
use assert_fs::prelude::*;

pub const UUID_1: &str = "550e8400-e29b-41d4-a716-446655440000";
pub const UUID_2: &str = "550e8400-e29b-41d4-a716-446655440001";
pub const UUID_3: &str = "550e8400-e29b-41d4-a716-446655440002";

pub fn md_with_frontmatter(frontmatter: &str, content: &str) -> String {
    format!("---\n{frontmatter}\n---\n{content}")
}

struct Note {
    id: String,
    content: String,
}

struct Session {
    id: String,
    notes: Vec<Note>,
    memo: Option<String>,
    has_meta: bool,
}

struct Folder {
    path: String,
    sessions: Vec<Session>,
    files: Vec<(String, String)>,
}

#[derive(Default)]
pub struct TestEnvBuilder {
    root_sessions: Vec<Session>,
    folders: Vec<Folder>,
    root_files: Vec<(String, String)>,
}

impl TestEnvBuilder {
    pub fn session(self, id: &str) -> SessionBuilder {
        SessionBuilder {
            parent: SessionParent::Root(self),
            session: Session {
                id: id.to_string(),
                notes: Vec::new(),
                memo: None,
                has_meta: true,
            },
        }
    }

    pub fn folder(self, path: &str) -> FolderBuilder {
        FolderBuilder {
            env: self,
            folder: Folder {
                path: path.to_string(),
                sessions: Vec::new(),
                files: Vec::new(),
            },
        }
    }

    pub fn file(mut self, name: &str, content: &str) -> Self {
        self.root_files
            .push((name.to_string(), content.to_string()));
        self
    }

    pub fn build(self) -> TestEnv {
        let temp = TempDir::new().unwrap();

        for (name, content) in &self.root_files {
            temp.child(name).write_str(content).unwrap();
        }

        for session in &self.root_sessions {
            write_session(&temp, "", session);
        }

        for folder in &self.folders {
            let folder_path = temp.child(&folder.path);
            folder_path.create_dir_all().unwrap();

            for (name, content) in &folder.files {
                folder_path.child(name).write_str(content).unwrap();
            }

            for session in &folder.sessions {
                write_session(&temp, &folder.path, session);
            }
        }

        TestEnv { temp }
    }

    fn add_root_session(mut self, session: Session) -> Self {
        self.root_sessions.push(session);
        self
    }

    fn add_folder(mut self, folder: Folder) -> Self {
        self.folders.push(folder);
        self
    }
}

fn write_session(temp: &TempDir, folder_path: &str, session: &Session) {
    let session_path = if folder_path.is_empty() {
        temp.child(&session.id)
    } else {
        temp.child(folder_path).child(&session.id)
    };
    session_path.create_dir_all().unwrap();

    if session.has_meta {
        session_path.child("_meta.json").write_str("{}").unwrap();
    }

    for note in &session.notes {
        let content = md_with_frontmatter(&format!("id: {}", note.id), &note.content);
        session_path
            .child(format!("{}.md", note.id))
            .write_str(&content)
            .unwrap();
    }

    if let Some(memo) = &session.memo {
        session_path.child("_memo.md").write_str(memo).unwrap();
    }
}

enum SessionParent {
    Root(TestEnvBuilder),
    Folder(FolderBuilder),
}

pub struct SessionBuilder {
    parent: SessionParent,
    session: Session,
}

impl SessionBuilder {
    pub fn note(mut self, id: &str, content: &str) -> Self {
        self.session.notes.push(Note {
            id: id.to_string(),
            content: content.to_string(),
        });
        self
    }

    pub fn memo(mut self, content: &str) -> Self {
        self.session.memo = Some(content.to_string());
        self
    }

    pub fn no_meta(mut self) -> Self {
        self.session.has_meta = false;
        self
    }

    pub fn done(self) -> TestEnvBuilder {
        match self.parent {
            SessionParent::Root(env) => env.add_root_session(self.session),
            SessionParent::Folder(_) => {
                panic!("Use done_folder() for sessions inside folders")
            }
        }
    }

    pub fn done_folder(self) -> FolderBuilder {
        match self.parent {
            SessionParent::Root(_) => {
                panic!("Use done() for root-level sessions")
            }
            SessionParent::Folder(mut folder) => {
                folder.folder.sessions.push(self.session);
                folder
            }
        }
    }
}

pub struct FolderBuilder {
    env: TestEnvBuilder,
    folder: Folder,
}

impl FolderBuilder {
    pub fn session(self, id: &str) -> SessionBuilder {
        SessionBuilder {
            parent: SessionParent::Folder(self),
            session: Session {
                id: id.to_string(),
                notes: Vec::new(),
                memo: None,
                has_meta: true,
            },
        }
    }

    pub fn file(mut self, name: &str, content: &str) -> Self {
        self.folder
            .files
            .push((name.to_string(), content.to_string()));
        self
    }

    pub fn done(self) -> TestEnvBuilder {
        self.env.add_folder(self.folder)
    }
}

pub struct TestEnv {
    temp: TempDir,
}

impl TestEnv {
    pub fn new() -> TestEnvBuilder {
        TestEnvBuilder::default()
    }

    pub fn path(&self) -> &Path {
        self.temp.path()
    }

    pub fn child(&self, path: &str) -> ChildPath {
        self.temp.child(path)
    }

    pub fn session_path(&self, id: &str) -> PathBuf {
        self.temp.path().join(id)
    }

    pub fn folder_session_path(&self, folder: &str, id: &str) -> PathBuf {
        self.temp.path().join(folder).join(id)
    }
}
