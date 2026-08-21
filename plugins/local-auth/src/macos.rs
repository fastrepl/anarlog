use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_foundation::{NSError, NSString};
use objc2_local_authentication::{LAContext, LAPolicy};
use tauri::Runtime;

use crate::Error;

const AUTH_TIMEOUT: Duration = Duration::from_secs(120);

pub fn available<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<bool, Error> {
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(policy_available());
    })?;
    rx.recv_timeout(Duration::from_secs(5))
        .map_err(|error| Error::Platform(error.to_string()))
}

pub fn authenticate<R: Runtime>(app: &tauri::AppHandle<R>, reason: &str) -> Result<bool, Error> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(Error::Platform(
            "authentication reason must not be empty".into(),
        ));
    }

    let (tx, rx) = mpsc::channel();
    let localized_reason = reason.to_string();
    app.run_on_main_thread(move || {
        start_authentication(&localized_reason, tx);
    })?;
    rx.recv_timeout(AUTH_TIMEOUT)
        .map_err(|error| Error::Platform(error.to_string()))?
}

fn policy_available() -> bool {
    let context = unsafe { LAContext::new() };
    unsafe { context.canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthentication) }.is_ok()
}

fn start_authentication(reason: &str, tx: mpsc::Sender<Result<bool, Error>>) {
    let context = unsafe { LAContext::new() };
    if unsafe { context.canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthentication) }.is_err() {
        let _ = tx.send(Ok(false));
        return;
    }

    let keep_alive = context.clone();
    let localized_reason = NSString::from_str(reason);
    let reply = RcBlock::new(move |success: Bool, _error: *mut NSError| {
        let _keep_alive = &keep_alive;
        let _ = tx.send(Ok(success.as_bool()));
    });

    unsafe {
        context.evaluatePolicy_localizedReason_reply(
            LAPolicy::DeviceOwnerAuthentication,
            &localized_reason,
            &reply,
        );
    }
}
