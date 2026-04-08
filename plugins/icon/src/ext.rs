#[cfg(target_os = "macos")]
mod overlay_state {
    use std::sync::Mutex;

    #[derive(Clone, Default)]
    pub struct State {
        pub original_icon_data: Option<Vec<u8>>,
        pub recording_active: bool,
        pub notification_count: Option<u8>,
    }

    static STATE: Mutex<State> = Mutex::new(State {
        original_icon_data: None,
        recording_active: false,
        notification_count: None,
    });

    pub fn get() -> State {
        STATE.lock().unwrap().clone()
    }

    pub fn update(update: impl FnOnce(&mut State)) -> State {
        let mut state = STATE.lock().unwrap();
        update(&mut state);
        state.clone()
    }
}

#[cfg(target_os = "macos")]
mod icon_helpers {
    use objc2_app_kit::NSImage;

    pub fn image_to_bytes(image: &NSImage) -> Option<Vec<u8>> {
        let tiff_data = image.TIFFRepresentation()?;
        let len = tiff_data.length();
        if len == 0 {
            return None;
        }
        let mut bytes = vec![0u8; len];
        unsafe {
            tiff_data.getBytes_length(
                std::ptr::NonNull::new(bytes.as_mut_ptr() as *mut std::ffi::c_void).unwrap(),
                len,
            );
        }
        Some(bytes)
    }
}

pub struct Icon<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    #[allow(dead_code)]
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Icon<'a, R, M> {
    pub fn set_dock_icon(&self, name: String) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            use std::path::PathBuf;
            use tauri::path::BaseDirectory;

            let icon_path = if cfg!(debug_assertions) {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("apps")
                    .join("desktop")
                    .join("src-tauri")
                    .join("icons")
                    .join(&name)
                    .join("icon.icns")
            } else {
                self.manager
                    .path()
                    .resolve(format!("icons/{}.icns", name), BaseDirectory::Resource)
                    .map_err(crate::Error::Tauri)?
            };

            if !icon_path.exists() {
                return Err(crate::Error::Custom(format!(
                    "Icon file not found: {}",
                    icon_path.display()
                )));
            }

            let icon_path_str = icon_path.to_string_lossy().to_string();

            let app_handle = self.manager.app_handle();
            app_handle
                .run_on_main_thread(move || {
                    use objc2::AnyThread;
                    use objc2_app_kit::{NSApplication, NSImage};
                    use objc2_foundation::{MainThreadMarker, NSString};

                    let mtm =
                        MainThreadMarker::new().expect("run_on_main_thread guarantees main thread");
                    let ns_app = NSApplication::sharedApplication(mtm);

                    let path_str = NSString::from_str(&icon_path_str);
                    let Some(image) = NSImage::initWithContentsOfFile(NSImage::alloc(), &path_str)
                    else {
                        return;
                    };

                    let Some(bytes) = icon_helpers::image_to_bytes(&image) else {
                        return;
                    };

                    let state = overlay_state::update(|state| {
                        state.original_icon_data = Some(bytes);
                    });

                    if let Some(composite_image) = compose_icon(&image, &state) {
                        unsafe { ns_app.setApplicationIconImage(Some(&composite_image)) };
                    } else {
                        unsafe { ns_app.setApplicationIconImage(Some(&image)) };
                    }
                })
                .map_err(crate::Error::Tauri)?;

            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = name;
            Ok(())
        }
    }

    pub fn reset_dock_icon(&self) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            let app_handle = self.manager.app_handle();
            app_handle
                .run_on_main_thread(move || {
                    use objc2_app_kit::NSApplication;
                    use objc2_foundation::MainThreadMarker;

                    let mtm =
                        MainThreadMarker::new().expect("run_on_main_thread guarantees main thread");
                    let ns_app = NSApplication::sharedApplication(mtm);

                    let state = overlay_state::update(|state| {
                        state.original_icon_data = None;
                    });
                    unsafe { ns_app.setApplicationIconImage(None) };

                    if state.recording_active || state.notification_count.is_some() {
                        let Some(current) = ns_app.applicationIconImage() else {
                            return;
                        };

                        if let Some(composite_image) = compose_icon(&current, &state) {
                            unsafe { ns_app.setApplicationIconImage(Some(&composite_image)) };
                        }
                    }
                })
                .map_err(crate::Error::Tauri)?;

            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            Ok(())
        }
    }

    pub fn set_recording_indicator(&self, show: bool) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            let app_handle = self.manager.app_handle();
            app_handle
                .run_on_main_thread(move || {
                    use objc2::AnyThread;
                    use objc2_app_kit::{NSApplication, NSImage};
                    use objc2_foundation::{MainThreadMarker, NSData};

                    let mtm =
                        MainThreadMarker::new().expect("run_on_main_thread guarantees main thread");
                    let ns_app = NSApplication::sharedApplication(mtm);

                    let state = overlay_state::get();
                    let base_image = if let Some(original_data) = state.original_icon_data.clone() {
                        let ns_data = NSData::with_bytes(&original_data);
                        match NSImage::initWithData(NSImage::alloc(), &ns_data) {
                            Some(image) => image,
                            None => return,
                        }
                    } else {
                        let Some(current) = ns_app.applicationIconImage() else {
                            if !show {
                                overlay_state::update(|state| {
                                    state.recording_active = false;
                                });
                            }
                            return;
                        };

                        if state.recording_active && show {
                            return;
                        }

                        let Some(bytes) = icon_helpers::image_to_bytes(&current) else {
                            return;
                        };

                        overlay_state::update(|state| {
                            state.original_icon_data = Some(bytes);
                        });

                        current
                    };

                    let state = overlay_state::update(|state| {
                        state.recording_active = show;
                    });

                    if let Some(composite_image) = compose_icon(&base_image, &state) {
                        unsafe { ns_app.setApplicationIconImage(Some(&composite_image)) };
                    } else if state.original_icon_data.is_some() {
                        unsafe { ns_app.setApplicationIconImage(Some(&base_image)) };
                    } else {
                        unsafe { ns_app.setApplicationIconImage(None) };
                    }
                })
                .map_err(crate::Error::Tauri)?;

            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = show;
            Ok(())
        }
    }

    pub fn set_notification_badge(&self, count: Option<u8>) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            let app_handle = self.manager.app_handle();
            app_handle
                .run_on_main_thread(move || {
                    use objc2::AnyThread;
                    use objc2_app_kit::{NSApplication, NSImage};
                    use objc2_foundation::{MainThreadMarker, NSData};

                    let mtm =
                        MainThreadMarker::new().expect("run_on_main_thread guarantees main thread");
                    let ns_app = NSApplication::sharedApplication(mtm);

                    let next_count = count.filter(|count| *count > 0);
                    let state = overlay_state::get();

                    let base_image = if let Some(original_data) = state.original_icon_data.clone() {
                        let ns_data = NSData::with_bytes(&original_data);
                        match NSImage::initWithData(NSImage::alloc(), &ns_data) {
                            Some(image) => image,
                            None => return,
                        }
                    } else {
                        let Some(current) = ns_app.applicationIconImage() else {
                            overlay_state::update(|state| {
                                state.notification_count = next_count;
                            });
                            return;
                        };

                        let Some(bytes) = icon_helpers::image_to_bytes(&current) else {
                            return;
                        };

                        overlay_state::update(|state| {
                            state.original_icon_data = Some(bytes);
                        });

                        current
                    };

                    let state = overlay_state::update(|state| {
                        state.notification_count = next_count;
                    });

                    if let Some(composite_image) = compose_icon(&base_image, &state) {
                        unsafe { ns_app.setApplicationIconImage(Some(&composite_image)) };
                    } else if state.original_icon_data.is_some() {
                        unsafe { ns_app.setApplicationIconImage(Some(&base_image)) };
                    } else {
                        unsafe { ns_app.setApplicationIconImage(None) };
                    }
                })
                .map_err(crate::Error::Tauri)?;

            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = count;
            Ok(())
        }
    }

    pub fn get_icon(&self) -> Result<Option<String>, crate::Error> {
        #[cfg(target_os = "macos")]
        {
            use base64::Engine;
            use objc2::AnyThread;
            use objc2::msg_send;
            use objc2_app_kit::{NSApplication, NSBitmapImageFileType, NSBitmapImageRep};
            use objc2_foundation::{MainThreadMarker, NSRect, NSSize};
            use std::sync::mpsc;

            let (tx, rx) = mpsc::channel();
            let app_handle = self.manager.app_handle();

            app_handle
                .run_on_main_thread(move || {
                    let mtm =
                        MainThreadMarker::new().expect("run_on_main_thread guarantees main thread");
                    let ns_app = NSApplication::sharedApplication(mtm);

                    unsafe {
                        let Some(image) = ns_app.applicationIconImage() else {
                            let _ = tx.send(None);
                            return;
                        };

                        let size = NSSize::new(64.0, 64.0);
                        image.setSize(size);

                        let mut rect = NSRect::new(objc2_foundation::NSPoint::new(0.0, 0.0), size);
                        let Some(cgimage) = image.CGImageForProposedRect_context_hints(
                            &mut rect as *mut NSRect as *mut _,
                            None,
                            None,
                        ) else {
                            let _ = tx.send(None);
                            return;
                        };

                        let bitmap =
                            NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &cgimage);

                        let Some(png_data) = bitmap.representationUsingType_properties(
                            NSBitmapImageFileType::PNG,
                            &objc2_foundation::NSDictionary::new(),
                        ) else {
                            let _ = tx.send(None);
                            return;
                        };

                        let len: usize = msg_send![&*png_data, length];
                        let ptr: *const u8 = msg_send![&*png_data, bytes];
                        let slice = std::slice::from_raw_parts(ptr, len);
                        let base64 = base64::engine::general_purpose::STANDARD.encode(slice);
                        let _ = tx.send(Some(base64));
                    }
                })
                .map_err(crate::Error::Tauri)?;

            rx.recv()
                .map_err(|e| crate::Error::Custom(format!("Failed to receive icon data: {}", e)))
        }

        #[cfg(not(target_os = "macos"))]
        {
            Ok(None)
        }
    }
}

#[cfg(target_os = "macos")]
fn compose_icon(
    base_image: &objc2_app_kit::NSImage,
    state: &overlay_state::State,
) -> Option<objc2::rc::Retained<objc2_app_kit::NSImage>> {
    if state.recording_active {
        return Some(crate::overlay::Overlay::Recording.draw(base_image));
    }

    state
        .notification_count
        .filter(|count| *count > 0)
        .map(|count| crate::overlay::Overlay::Notification(count).draw(base_image))
}

pub trait IconPluginExt<R: tauri::Runtime> {
    fn icon(&self) -> Icon<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> IconPluginExt<R> for T {
    fn icon(&self) -> Icon<'_, R, Self>
    where
        Self: Sized,
    {
        Icon {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
