use std::sync::{
    Arc,
    atomic::{AtomicI8, Ordering},
};

use hypr_download_interface::DownloadProgress;

use crate::model::DownloadableModel;
use crate::runtime::ModelDownloaderRuntime;

pub(crate) fn make_progress_callback<M: DownloadableModel>(
    runtime: Arc<dyn ModelDownloaderRuntime<M>>,
    model: M,
) -> impl Fn(DownloadProgress) + Send + Sync {
    let last = Arc::new(AtomicI8::new(-1));

    move |progress: DownloadProgress| match progress {
        DownloadProgress::Started => {
            last.store(0, Ordering::Relaxed);
            runtime.emit_progress(&model, 0);
        }
        DownloadProgress::Progress(downloaded, total_size) => {
            if total_size == 0 {
                return;
            }

            let percent = (downloaded as f64 / total_size as f64) * 100.0;
            let current = (percent.floor().clamp(0.0, 99.0) as i16) as i8;

            let mut prev = last.load(Ordering::Relaxed);
            while current > prev {
                match last.compare_exchange_weak(
                    prev,
                    current,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        runtime.emit_progress(&model, current);
                        break;
                    }
                    Err(p) => prev = p,
                }
            }
        }
        DownloadProgress::Finished => {
            let prev = last.swap(99, Ordering::Relaxed);
            if prev < 99 {
                runtime.emit_progress(&model, 99);
            }
        }
    }
}
