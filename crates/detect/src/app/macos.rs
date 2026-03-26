use cidre::{blocks, ns, ns::workspace::notification as wsn, objc::Obj};
use tokio::time::{Duration, sleep};

use crate::BackgroundTask;

// `defaults read /Applications/Char.app/Contents/Info.plist CFBundleIdentifier`
const MEETING_APP_LIST: [&str; 3] = [
    "us.zoom.xos",         // tested
    "Cisco-Systems.Spark", // tested
    "com.microsoft.teams",
];

pub struct Detector {
    background: BackgroundTask,
}

impl Default for Detector {
    fn default() -> Self {
        Self {
            background: BackgroundTask::default(),
        }
    }
}

impl crate::Observer for Detector {
    fn start(&mut self, _f: crate::DetectCallback) {
        self.background.start(|running, mut rx| async move {
            let notification_running = running.clone();
            let block = move |n: &ns::Notification| {
                if !notification_running.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }

                let user_info = n.user_info().unwrap();

                if let Some(app) = user_info.get(wsn::app_key()) {
                    if let Some(app) = app.try_cast(ns::RunningApp::cls()) {
                        let bundle_id = app.bundle_id().unwrap().to_string();
                        let detected = MEETING_APP_LIST.contains(&bundle_id.as_str());
                        if detected {
                            // f(DetectEvent::MeetingAppStarted(bundle_id));
                        }
                    }
                }
            };

            let mut block = blocks::SyncBlock::new1(block);
            let notifications = [wsn::did_launch_app()];

            let mut observers = Vec::new();
            let mut nc = ns::Workspace::shared().notification_center();

            for name in notifications {
                let observer = nc.add_observer_block(name, None, None, &mut block);
                observers.push(observer);
            }

            loop {
                tokio::select! {
                    _ = &mut rx => {
                        break;
                    }
                    _ = sleep(Duration::from_millis(500)) => {
                        if !running.load(std::sync::atomic::Ordering::SeqCst) {
                            break;
                        }
                    }
                }
            }

            let mut nc = ns::Workspace::shared().notification_center();
            for observer in observers {
                nc.remove_observer(&observer);
            }
        });
    }

    fn stop(&mut self) {
        self.background.stop();
    }
}
