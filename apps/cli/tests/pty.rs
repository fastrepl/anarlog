use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use vt100::Parser;

const PTY_ROWS: u16 = 24;
const PTY_COLS: u16 = 80;
const POLL_INTERVAL: Duration = Duration::from_millis(30);

struct PtyApp {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    output: Arc<Mutex<Vec<u8>>>,
    screen: Arc<Mutex<Parser>>,
    reader_handle: Option<std::thread::JoinHandle<()>>,
    _temp_dir: PathBuf,
}

impl PtyApp {
    #[allow(deprecated)]
    fn spawn(args: &[&str]) -> Self {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let temp_path = temp_dir.into_path();

        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: PTY_ROWS,
                cols: PTY_COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("failed to open pty");

        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_char"));
        for arg in args {
            cmd.arg(arg);
        }

        cmd.env("TERM", "xterm-256color");
        cmd.env("NO_COLOR", "1");
        cmd.env("XDG_CONFIG_HOME", temp_path.join("config"));
        cmd.env("XDG_DATA_HOME", temp_path.join("data"));
        cmd.env("XDG_CACHE_HOME", temp_path.join("cache"));
        cmd.env("HYPR_ANALYTICS_DISABLED", "1");
        cmd.env("HYPR_ENV", "test");

        let child = pair.slave.spawn_command(cmd).expect("failed to spawn");
        drop(pair.slave);

        let writer = pair.master.take_writer().expect("failed to take writer");
        let mut reader = pair
            .master
            .try_clone_reader()
            .expect("failed to clone reader");

        let output: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let output_clone = Arc::clone(&output);

        let screen = Arc::new(Mutex::new(Parser::new(PTY_ROWS, PTY_COLS, 0)));
        let screen_clone = Arc::clone(&screen);

        let reader_handle = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        {
                            let mut out = output_clone.lock().unwrap();
                            out.extend_from_slice(&buf[..n]);
                        }
                        {
                            let mut scr = screen_clone.lock().unwrap();
                            scr.process(&buf[..n]);
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        PtyApp {
            child,
            writer,
            output,
            screen,
            reader_handle: Some(reader_handle),
            _temp_dir: temp_path,
        }
    }

    fn screen_text(&self) -> String {
        let scr = self.screen.lock().unwrap();
        scr.screen().contents()
    }

    fn raw_output(&self) -> String {
        let out = self.output.lock().unwrap();
        String::from_utf8_lossy(&out).into_owned()
    }

    fn wait_for_screen(&self, needle: &str, timeout: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            let text = self.screen_text();
            if !text.is_empty() && text.contains(needle) {
                return true;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        false
    }
}

impl Drop for PtyApp {
    fn drop(&mut self) {
        let _ = self.writer.write_all(b"\x03");
        let _ = self.writer.flush();

        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(500) {
            if let Ok(Some(_)) = self.child.try_wait() {
                break;
            }
            std::thread::sleep(POLL_INTERVAL);
        }

        let _ = self.child.kill();
        let _ = self.child.wait();

        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }

        let _ = std::fs::remove_dir_all(&self._temp_dir);

        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg_attr(not(feature = "e2e"), ignore)]
#[test]
fn entry_screen_renders() {
    let app = PtyApp::spawn(&[]);
    assert!(
        app.wait_for_screen("Tip:", Duration::from_secs(5)),
        "entry screen should render with 'Tip:' text.\nScreen:\n{}\nRaw:\n{}",
        app.screen_text(),
        app.raw_output()
    );
}

#[cfg_attr(not(feature = "e2e"), ignore)]
#[test]
fn connect_screen_renders() {
    let app = PtyApp::spawn(&["connect"]);
    assert!(
        app.wait_for_screen("Connect a provider", Duration::from_secs(5)),
        "connect screen should render.\nScreen:\n{}\nRaw:\n{}",
        app.screen_text(),
        app.raw_output()
    );
}
