use std::io;

fn main() {
    let mut stdout = io::stdout().lock();
    let Some(origin) = chrome_native_host::extension_origin(std::env::args().skip(1)) else {
        eprintln!("expected to be started by Chrome for a chrome-extension:// origin");
        std::process::exit(2);
    };
    let Some(dir) = chrome_native_host::default_endpoint_dir() else {
        eprintln!("data directory not found");
        std::process::exit(1);
    };
    #[cfg(unix)]
    let browser_pid = Some(std::os::unix::process::parent_id());
    #[cfg(not(unix))]
    let browser_pid = None;
    if let Err(error) = chrome_native_host::run(io::stdin(), &mut stdout, &dir, &origin, browser_pid)
    {
        eprintln!("native host stopped: {error}");
    }
}
