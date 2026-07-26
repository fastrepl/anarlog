use std::{
    ffi::c_void,
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicPtr, Ordering},
    },
    thread::{self, JoinHandle},
};

use crate::{
    hotkey::{Modifier, Modifiers},
    key_event::{KVK_FUNCTION, KeyEvent},
};

#[derive(Debug, thiserror::Error)]
pub enum TapError {
    #[error("failed to create CGEventTap (check Accessibility/Input Monitoring permissions)")]
    TapCreate,
}

#[derive(Debug, Clone, Copy)]
pub enum TapEvent {
    Key(KeyEvent),
    MouseClick,
}

type UserCallback = Arc<dyn Fn(TapEvent) + Send + Sync>;

pub struct EventTap {
    stop_flag: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl EventTap {
    pub fn start<F>(callback: F) -> Result<Self, TapError>
    where
        F: Fn(TapEvent) + Send + Sync + 'static,
    {
        let cb: UserCallback = Arc::new(callback);
        let stop_flag = Arc::new(AtomicBool::new(false));

        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), TapError>>();
        let stop_flag_thread = stop_flag.clone();
        let thread = thread::Builder::new()
            .name("shortcut-macos-event-tap".into())
            .spawn(move || run_tap_thread(cb, stop_flag_thread, ready_tx))
            .expect("spawn event tap thread");

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                stop_flag,
                thread: Some(thread),
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(TapError::TapCreate),
        }
    }

    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for EventTap {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

struct TapContext {
    callback: UserCallback,
    tap_port: AtomicPtr<c_void>,
    fn_pressed: AtomicBool,
}

fn run_tap_thread(
    callback: UserCallback,
    stop_flag: Arc<AtomicBool>,
    ready_tx: std::sync::mpsc::Sender<Result<(), TapError>>,
) {
    let ctx = Box::new(TapContext {
        callback,
        tap_port: AtomicPtr::new(ptr::null_mut()),
        fn_pressed: AtomicBool::new(false),
    });
    let ctx_ptr = Box::into_raw(ctx);

    unsafe {
        let event_mask: u64 = (1 << KCG_EVENT_KEY_DOWN)
            | (1 << KCG_EVENT_KEY_UP)
            | (1 << KCG_EVENT_FLAGS_CHANGED)
            | (1 << KCG_EVENT_LEFT_MOUSE_DOWN)
            | (1 << KCG_EVENT_RIGHT_MOUSE_DOWN)
            | (1 << KCG_EVENT_OTHER_MOUSE_DOWN);

        let tap = CGEventTapCreate(
            KCG_HID_EVENT_TAP,
            KCG_HEAD_INSERT_EVENT_TAP,
            KCG_EVENT_TAP_OPTION_DEFAULT,
            event_mask,
            tap_callback,
            ctx_ptr as *mut c_void,
        );

        if tap.is_null() {
            let _ = ready_tx.send(Err(TapError::TapCreate));
            let _ = Box::from_raw(ctx_ptr);
            return;
        }
        (*ctx_ptr).tap_port.store(tap, Ordering::SeqCst);

        let source = CFMachPortCreateRunLoopSource(ptr::null(), tap, 0);
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);

        let _ = ready_tx.send(Ok(()));

        while !stop_flag.load(Ordering::SeqCst) {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.2, false);
        }

        CGEventTapEnable(tap, false);
        CFRunLoopRemoveSource(run_loop, source, kCFRunLoopCommonModes);
        CFRelease(source as *const c_void);
        CFRelease(tap as *const c_void);
        drop(Box::from_raw(ctx_ptr));
    }
}

extern "C" fn tap_callback(
    _proxy: *mut c_void,
    event_type: u32,
    event: *mut c_void,
    user_info: *mut c_void,
) -> *mut c_void {
    let ctx = unsafe { &*(user_info as *const TapContext) };

    match event_type {
        KCG_EVENT_TAP_DISABLED_BY_TIMEOUT | KCG_EVENT_TAP_DISABLED_BY_USER_INPUT => {
            let tap = ctx.tap_port.load(Ordering::SeqCst);
            if !tap.is_null() {
                unsafe { CGEventTapEnable(tap, true) };
            }
            return event;
        }
        KCG_EVENT_LEFT_MOUSE_DOWN | KCG_EVENT_RIGHT_MOUSE_DOWN | KCG_EVENT_OTHER_MOUSE_DOWN => {
            (ctx.callback)(TapEvent::MouseClick);
            return event;
        }
        KCG_EVENT_KEY_DOWN | KCG_EVENT_KEY_UP | KCG_EVENT_FLAGS_CHANGED => {}
        _ => return event,
    }

    unsafe {
        let keycode = CGEventGetIntegerValueField(event, KCG_KEYBOARD_EVENT_KEYCODE) as u16;
        let flags = CGEventGetFlags(event);

        if event_type == KCG_EVENT_FLAGS_CHANGED && keycode == KVK_FUNCTION {
            ctx.fn_pressed
                .store(flags & FLAG_SECONDARY_FN != 0, Ordering::SeqCst);
        }
        let fn_is_pressed = ctx.fn_pressed.load(Ordering::SeqCst);

        let key = if event_type == KCG_EVENT_KEY_DOWN {
            Some(keycode)
        } else {
            None
        };
        let modifiers = decode_flags(flags, fn_is_pressed);
        (ctx.callback)(TapEvent::Key(KeyEvent::new(key, modifiers)));
    }

    event
}

fn decode_flags(flags: u64, fn_pressed: bool) -> Modifiers {
    let mut m = Modifiers::empty();
    if flags & FLAG_COMMAND != 0 {
        m.insert(Modifier::Command);
    }
    if flags & FLAG_ALTERNATE != 0 {
        m.insert(Modifier::Option);
    }
    if flags & FLAG_SHIFT != 0 {
        m.insert(Modifier::Shift);
    }
    if flags & FLAG_CONTROL != 0 {
        m.insert(Modifier::Control);
    }
    if fn_pressed {
        m.insert(Modifier::Fn);
    }
    m
}

const KCG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
const KCG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
const KCG_EVENT_KEY_DOWN: u32 = 10;
const KCG_EVENT_KEY_UP: u32 = 11;
const KCG_EVENT_FLAGS_CHANGED: u32 = 12;
const KCG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
const KCG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const KCG_EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;

const KCG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

const KCG_HID_EVENT_TAP: u32 = 0;
const KCG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const KCG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

const FLAG_SHIFT: u64 = 1 << 17;
const FLAG_CONTROL: u64 = 1 << 18;
const FLAG_ALTERNATE: u64 = 1 << 19;
const FLAG_COMMAND: u64 = 1 << 20;
const FLAG_SECONDARY_FN: u64 = 1 << 23;

type CGEventTapCallBack = extern "C" fn(
    proxy: *mut c_void,
    event_type: u32,
    event: *mut c_void,
    user_info: *mut c_void,
) -> *mut c_void;

#[link(name = "CoreGraphics", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> *mut c_void;

    fn CGEventTapEnable(tap: *mut c_void, enable: bool);

    fn CGEventGetFlags(event: *mut c_void) -> u64;
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;

    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: *mut c_void,
        order: isize,
    ) -> *mut c_void;

    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRemoveSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRunInMode(
        mode: *const c_void,
        seconds: f64,
        return_after_source_handled: bool,
    ) -> i32;
    fn CFRelease(cf: *const c_void);

    static kCFRunLoopCommonModes: *const c_void;
    static kCFRunLoopDefaultMode: *const c_void;
}
