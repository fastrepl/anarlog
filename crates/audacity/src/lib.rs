use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const COMMANDS_FILENAME: &str = "audacity_commands.txt";
const SCRIPT_FILENAME: &str = "open_in_audacity.py";

#[derive(Debug, Clone)]
pub struct Track {
    path: PathBuf,
    name: Option<String>,
    muted: bool,
}

impl Track {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            name: None,
            muted: false,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    fn import_command(&self) -> io::Result<String> {
        Ok(format!("Import2: Filename={}", audacity_path(&self.path)?))
    }

    fn push_status_commands(&self, track_index: usize, commands: &mut Vec<String>) {
        commands.push(select_tracks_command(track_index, 1));
        if let Some(name) = &self.name {
            commands.push(format!("SetTrackStatus: Name={}", audacity_value(name)));
        }
        if self.muted {
            commands.push("SetTrackAudio: Mute=1".to_string());
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Project {
    tracks: Vec<Track>,
    align_start_to_zero: bool,
}

impl Project {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            align_start_to_zero: true,
        }
    }

    pub fn with_track(mut self, track: Track) -> Self {
        self.tracks.push(track);
        self
    }

    pub fn with_align_start_to_zero(mut self, align_start_to_zero: bool) -> Self {
        self.align_start_to_zero = align_start_to_zero;
        self
    }

    pub fn write_bundle(&self, out_dir: impl AsRef<Path>) -> io::Result<Bundle> {
        let out_dir = out_dir.as_ref();
        fs::create_dir_all(out_dir)?;

        let commands_path = out_dir.join(COMMANDS_FILENAME);
        let script_path = out_dir.join(SCRIPT_FILENAME);

        fs::write(&commands_path, self.render_commands()?)?;
        fs::write(&script_path, PYTHON_HELPER)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&script_path)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script_path, permissions)?;
        }

        Ok(Bundle {
            commands_path,
            script_path,
        })
    }

    fn render_commands(&self) -> io::Result<String> {
        let commands = self.render_command_lines()?;
        if commands.is_empty() {
            return Ok(String::new());
        }

        Ok(format!("{}\n", commands.join("\n")))
    }

    fn render_command_lines(&self) -> io::Result<Vec<String>> {
        let mut commands = Vec::with_capacity(self.tracks.len() * 3 + 2);

        for track in &self.tracks {
            commands.push(track.import_command()?);
        }

        for (index, track) in self.tracks.iter().enumerate() {
            track.push_status_commands(index, &mut commands);
        }

        if self.should_align_start_to_zero() {
            commands.push(select_tracks_command(0, self.tracks.len()));
            commands.push("Align_StartToZero:".to_string());
        }

        Ok(commands)
    }

    fn should_align_start_to_zero(&self) -> bool {
        self.align_start_to_zero && !self.tracks.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct Bundle {
    pub commands_path: PathBuf,
    pub script_path: PathBuf,
}

fn audacity_path(path: &Path) -> io::Result<String> {
    let path = path.canonicalize()?;
    Ok(audacity_value(&path.to_string_lossy()))
}

fn audacity_value(value: &str) -> String {
    format!("\"{}\"", escape_audacity_value(value))
}

fn escape_audacity_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn select_tracks_command(track_index: usize, track_count: usize) -> String {
    format!("SelectTracks: Track={track_index} TrackCount={track_count} Mode=Set")
}

const PYTHON_HELPER: &str = r##"#!/usr/bin/env python3
import os
import queue
import signal
import threading
from pathlib import Path


def find_pipe_paths():
    env_to = os.environ.get("AUDACITY_PIPE_TO")
    env_from = os.environ.get("AUDACITY_PIPE_FROM")
    if env_to and env_from:
        return Path(env_to), Path(env_from)

    uid = os.getuid()
    candidates = [
        (Path(f"/tmp/audacity_script_pipe.to.{uid}"), Path(f"/tmp/audacity_script_pipe.from.{uid}")),
        (Path("/tmp/audacity_script_pipe.to"), Path("/tmp/audacity_script_pipe.from")),
    ]

    for to_path, from_path in candidates:
        if to_path.exists() and from_path.exists():
            return to_path, from_path

    raise SystemExit(
        "Audacity pipe not found.\n"
        "1. Open Audacity.\n"
        "2. Go to Audacity > Preferences > Modules.\n"
        "3. Set mod-script-pipe to Enabled.\n"
        "4. Restart Audacity.\n"
        "5. Verify the pipes exist with:\n"
        "   find /tmp /private/tmp -maxdepth 1 -name 'audacity_script_pipe*'\n"
        "6. If Audacity uses non-default names, set AUDACITY_PIPE_TO and "
        "AUDACITY_PIPE_FROM before rerunning this helper."
    )


def open_pipe(path, mode, timeout=3.0):
    result = queue.Queue(maxsize=1)

    def worker():
        try:
            pipe = path.open(mode, encoding="utf-8", errors="replace")
        except Exception as exc:
            result.put(exc)
            return
        result.put(pipe)

    thread = threading.Thread(target=worker, daemon=True)
    thread.start()
    thread.join(timeout)
    if thread.is_alive():
        raise SystemExit(
            f"Timed out opening {path}. Make sure Audacity is running and mod-script-pipe is enabled."
        )

    opened = result.get()
    if isinstance(opened, Exception):
        raise SystemExit(f"Failed to open {path}: {opened}")
    return opened


def read_response(from_pipe):
    response = []
    while True:
        line = from_pipe.readline()
        if line == "":
            raise SystemExit("Audacity pipe closed while waiting for a response.")

        line = line.rstrip("\r\n")
        if not line:
            return response

        print(f"<- {line}")
        response.append(line)


def send_command(to_pipe, from_pipe, command):
    print(f"-> {command}")
    to_pipe.write(command + "\n")
    to_pipe.flush()

    response = read_response(from_pipe)
    if not response:
        raise SystemExit(f"No response from Audacity for command: {command}")

    status_line = response[-1]
    if status_line.endswith("finished: OK"):
        return

    raise SystemExit(
        "Audacity command failed:\n"
        f"  command: {command}\n"
        f"  status: {status_line}"
    )


def exit_now(code):
    os._exit(code)


def main():
    command_path = Path(__file__).with_name("audacity_commands.txt")
    if not command_path.exists():
        raise SystemExit(f"missing command file: {command_path}")

    to_path, from_path = find_pipe_paths()
    commands = [
        line.strip()
        for line in command_path.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]

    signal.signal(signal.SIGINT, lambda *_: exit_now(130))

    with open_pipe(to_path, "w") as to_pipe, open_pipe(from_path, "r") as from_pipe:
        for command in commands:
            send_command(to_pipe, from_pipe, command)


if __name__ == "__main__":
    try:
        main()
    except BrokenPipeError:
        raise SystemExit("Audacity pipe closed while sending commands.")
"##;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn render_command_lines_renders_tracks_and_alignment() {
        let temp_dir = unique_test_dir("render-command-lines");
        fs::create_dir_all(&temp_dir).unwrap();

        let first_path = temp_dir.join("one.wav");
        let second_path = temp_dir.join("two.wav");
        fs::write(&first_path, []).unwrap();
        fs::write(&second_path, []).unwrap();

        let project = Project::new()
            .with_track(Track::new(&first_path).with_name("lead"))
            .with_track(Track::new(&second_path).with_name("backing").muted(true));

        let commands = project.render_command_lines().unwrap();

        assert_eq!(
            commands,
            vec![
                format!("Import2: Filename={}", audacity_path(&first_path).unwrap()),
                format!("Import2: Filename={}", audacity_path(&second_path).unwrap()),
                "SelectTracks: Track=0 TrackCount=1 Mode=Set".to_string(),
                "SetTrackStatus: Name=\"lead\"".to_string(),
                "SelectTracks: Track=1 TrackCount=1 Mode=Set".to_string(),
                "SetTrackStatus: Name=\"backing\"".to_string(),
                "SetTrackAudio: Mute=1".to_string(),
                "SelectTracks: Track=0 TrackCount=2 Mode=Set".to_string(),
                "Align_StartToZero:".to_string(),
            ]
        );

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn render_commands_is_empty_for_empty_projects() {
        assert_eq!(Project::new().render_commands().unwrap(), "");
    }

    #[test]
    fn audacity_value_escapes_quotes_and_backslashes() {
        assert_eq!(
            audacity_value(r#"say "hello" from c:\tmp"#),
            r#""say \"hello\" from c:\\tmp""#
        );
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("audacity-{label}-{}-{nanos}", std::process::id()))
    }
}
