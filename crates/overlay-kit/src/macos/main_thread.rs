use dispatch2::DispatchQueue;
use objc2_foundation::NSThread;

pub fn run_on_main_thread<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    if NSThread::isMainThread_class() {
        return f();
    }

    let mut result = None;
    DispatchQueue::main().exec_sync(|| {
        result = Some(f());
    });
    result.expect("main-thread work completed")
}
