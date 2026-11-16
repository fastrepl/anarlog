//! Device-owner authentication for the app lock, like the `local-auth` plugin: Touch ID or
//! the account password through LocalAuthentication on macOS. The host installs
//! [`NativeDeviceAuth`]; without it authentication is unavailable.

use std::sync::Arc;

use futures::channel::oneshot;

/// `DEVICE_AUTH_REASON` in `lock/auth.ts`.
pub const CHANGE_LOCK_SETTINGS: &str = "change lock settings";

pub trait DeviceAuth: Send + Sync {
    fn available(&self) -> bool;
    /// Resolves `true` only when the device owner authenticated.
    fn authenticate(&self, reason: &str) -> oneshot::Receiver<bool>;
}

#[derive(Clone)]
pub struct Installed(pub Arc<dyn DeviceAuth>);

impl gpui::Global for Installed {}

pub fn available(cx: &gpui::App) -> bool {
    cx.try_global::<Installed>()
        .is_some_and(|auth| auth.0.available())
}

pub fn authenticate(reason: &str, cx: &gpui::App) -> oneshot::Receiver<bool> {
    match cx.try_global::<Installed>() {
        Some(auth) => auth.0.authenticate(reason),
        None => {
            let (sender, receiver) = oneshot::channel();
            let _ = sender.send(false);
            receiver
        }
    }
}

pub struct NativeDeviceAuth;

#[cfg(target_os = "macos")]
#[link(name = "LocalAuthentication", kind = "framework")]
unsafe extern "C" {}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::{
        class, msg_send,
        rc::Retained,
        runtime::{AnyObject, Bool},
    };

    /// `LAPolicyDeviceOwnerAuthentication`: biometrics, a watch, or the account password.
    const DEVICE_OWNER: isize = 2;

    pub fn context() -> Option<Retained<AnyObject>> {
        unsafe { msg_send![class!(LAContext), new] }
    }

    pub fn can_evaluate(context: &AnyObject) -> bool {
        let mut error: *mut AnyObject = std::ptr::null_mut();
        unsafe { msg_send![context, canEvaluatePolicy: DEVICE_OWNER, error: &mut error] }
    }

    pub fn evaluate(context: &AnyObject, reason: &str, reply: impl Fn(bool) + 'static) {
        let Ok(reason) = std::ffi::CString::new(reason) else {
            return reply(false);
        };
        let block =
            block2::RcBlock::new(move |success: Bool, _: *mut AnyObject| reply(success.as_bool()));
        unsafe {
            let reason: *mut AnyObject =
                msg_send![class!(NSString), stringWithUTF8String: reason.as_ptr()];
            let _: () = msg_send![
                context,
                evaluatePolicy: DEVICE_OWNER,
                localizedReason: reason,
                reply: &*block
            ];
        }
    }
}

impl DeviceAuth for NativeDeviceAuth {
    fn available(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            macos::context().is_some_and(|context| macos::can_evaluate(&context))
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    fn authenticate(&self, reason: &str) -> oneshot::Receiver<bool> {
        let (sender, receiver) = oneshot::channel();
        #[cfg(target_os = "macos")]
        {
            let sender = std::sync::Mutex::new(Some(sender));
            let reply = move |success| {
                if let Some(sender) = sender.lock().ok().and_then(|mut sender| sender.take()) {
                    let _ = sender.send(success);
                }
            };
            match macos::context() {
                Some(context) if macos::can_evaluate(&context) => {
                    // The context must outlive the evaluation; the reply block keeps it.
                    let keep = context.clone();
                    macos::evaluate(&context, reason.trim(), move |success| {
                        let _ = &keep;
                        reply(success)
                    });
                }
                _ => reply(false),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (reason, sender.send(false));
        }
        receiver
    }
}
