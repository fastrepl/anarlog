use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use futures::channel::oneshot;
use gpui::TestAppContext;

use super::*;

/// Device authentication whose prompts the test answers one by one.
#[derive(Clone, Default)]
struct FakeAuth(Arc<Mutex<VecDeque<oneshot::Sender<bool>>>>);

impl FakeAuth {
    fn prompts(&self) -> usize {
        self.0.lock().unwrap().len()
    }

    fn answer(&self, approved: bool, cx: &mut TestAppContext) {
        let prompt = self
            .0
            .lock()
            .unwrap()
            .pop_front()
            .expect("a prompt is open");
        let _ = prompt.send(approved);
        cx.run_until_parked();
    }
}

impl device_auth::DeviceAuth for FakeAuth {
    fn available(&self) -> bool {
        true
    }

    fn authenticate(&self, _: &str) -> oneshot::Receiver<bool> {
        let (sender, receiver) = oneshot::channel();
        self.0.lock().unwrap().push_back(sender);
        receiver
    }
}

fn install_fake(enabled: bool, locked_notes: &[&str], cx: &mut TestAppContext) -> FakeAuth {
    let auth = FakeAuth::default();
    cx.update(|cx| {
        cx.set_global(device_auth::Installed(Arc::new(auth.clone())));
        cx.set_global(AppLock {
            available: true,
            locked_notes: Some(
                locked_notes
                    .iter()
                    .map(|id| SessionId((*id).into()))
                    .collect(),
            ),
            ..Default::default()
        });
        set_enabled(Some(enabled), cx);
    });
    cx.run_until_parked();
    auth
}

#[gpui::test]
fn hiding_the_main_window_relocks_and_a_stale_prompt_cannot_unlock(cx: &mut TestAppContext) {
    let auth = install_fake(true, &["s-locked"], cx);
    let note = SessionId("s-locked".into());
    // The launch prompt opens by itself.
    assert_eq!(auth.prompts(), 1);
    auth.answer(true, cx);
    cx.update(|cx| {
        assert!(!app_locked(cx));
        reveal_note(note.clone(), cx).detach();
    });
    auth.answer(true, cx);
    cx.read(|cx| assert_eq!(note_access(&note, cx), NoteAccess::Open));

    // Hiding locks without prompting and hides revealed notes again.
    cx.update(main_hidden);
    cx.read(|cx| {
        assert!(app_locked(cx));
        assert_eq!(note_access(&note, cx), NoteAccess::Locked);
    });
    assert_eq!(auth.prompts(), 0);

    // Showing prompts; hiding before it is answered makes that answer worthless, and the
    // next show prompts again once it settles.
    cx.update(main_shown);
    assert_eq!(auth.prompts(), 1);
    cx.update(main_hidden);
    cx.update(main_shown);
    auth.answer(true, cx);
    cx.read(|cx| assert!(app_locked(cx)));
    assert_eq!(auth.prompts(), 1);
    auth.answer(true, cx);
    cx.read(|cx| assert!(!app_locked(cx)));
}

#[gpui::test]
fn locked_notes_stay_hidden_until_the_owner_authenticates(cx: &mut TestAppContext) {
    let auth = install_fake(false, &["s-locked"], cx);
    let note = SessionId("s-locked".into());
    // Without "Lock app" nothing prompts at launch, but the locked note is still gated.
    assert_eq!(auth.prompts(), 0);
    cx.read(|cx| {
        assert!(!app_locked(cx));
        assert_eq!(note_access(&note, cx), NoteAccess::Locked);
        assert_eq!(
            note_access(&SessionId("s-open".into()), cx),
            NoteAccess::Open
        );
    });

    let declined = cx.update(|cx| reveal_note(note.clone(), cx));
    auth.answer(false, cx);
    assert!(!futures::executor::block_on(declined));
    cx.read(|cx| assert_eq!(note_access(&note, cx), NoteAccess::Locked));

    let approved = cx.update(|cx| reveal_note(note.clone(), cx));
    auth.answer(true, cx);
    assert!(futures::executor::block_on(approved));
    cx.read(|cx| assert_eq!(note_access(&note, cx), NoteAccess::Open));
}

#[gpui::test]
fn the_lock_setting_loads_when_the_library_opens_after_the_window(cx: &mut TestAppContext) {
    use desktop_runtime::Profile;
    use futures::executor::block_on;

    cx.executor().allow_parking();
    let directory = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: directory.path().join("library.sqlite"),
    };
    let (runtime, ready) = RuntimeHandle::start(profile.clone()).unwrap();
    block_on(ready.receive()).unwrap();
    let stored = block_on(async {
        let watch = settings::watch(&runtime).unwrap().receive().await.unwrap();
        let snapshot = settings::decode(&watch.snapshots.borrow().rows).unwrap();
        watch.unsubscribe().await.unwrap();
        snapshot
    });
    let base = stored["lock_app"].clone();
    block_on(
        settings::save(&runtime, "lock_app".into(), base, Value::Bool(true))
            .unwrap()
            .receive(),
    )
    .unwrap();
    block_on(runtime.shutdown()).unwrap();

    // The main window installs the lock while the runtime is still opening the library.
    let (runtime, _ready) = RuntimeHandle::start(profile).unwrap();
    let auth = FakeAuth::default();
    cx.update(|cx| {
        cx.set_global(device_auth::Installed(Arc::new(auth.clone())));
        cx.set_global(AppLock {
            available: true,
            ..Default::default()
        });
        watch(&runtime, cx);
    });
    for _ in 0..400 {
        cx.run_until_parked();
        if cx.read(|cx| app_locked(cx) && note_lock_known(cx)) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    cx.read(|cx| {
        assert!(app_locked(cx), "the library must stay locked");
        assert!(note_lock_known(cx), "note locks must load");
    });
    block_on(runtime.shutdown()).unwrap();
}
