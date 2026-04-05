mod common;

use tauri_plugin_icon::overlay::Overlay;

use std::time::Duration;

fn main() {
    common::run_app(|| {
        std::thread::sleep(Duration::from_millis(200));

        use objc2::AnyThread;
        use objc2::msg_send;
        use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
        use objc2_foundation::{NSDictionary, NSRect, NSSize, NSString};

        let icon_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("apps/desktop/src-tauri/icons/stable/icon.png");

        let path_str = NSString::from_str(&icon_path.to_string_lossy());
        let base_image = NSImage::initWithContentsOfFile(NSImage::alloc(), &path_str)
            .expect("failed to load base icon");

        for (overlay, name) in [
            (Overlay::Recording, "recording"),
            (Overlay::Degraded, "degraded"),
        ] {
            let result = overlay.draw(&base_image);

            unsafe {
                let size = NSSize::new(128.0, 128.0);
                result.setSize(size);

                let mut rect = NSRect::new(objc2_foundation::NSPoint::new(0.0, 0.0), size);
                let cgimage = result
                    .CGImageForProposedRect_context_hints(
                        &mut rect as *mut NSRect as *mut _,
                        None,
                        None,
                    )
                    .expect("failed to get CGImage");

                let bitmap = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &cgimage);

                let png_data = bitmap
                    .representationUsingType_properties(
                        NSBitmapImageFileType::PNG,
                        &NSDictionary::new(),
                    )
                    .expect("failed to create PNG");

                let len: usize = msg_send![&*png_data, length];
                let ptr: *const u8 = msg_send![&*png_data, bytes];
                let slice = std::slice::from_raw_parts(ptr, len);

                let out_path = std::env::temp_dir().join(format!("icon_overlay_{name}.png"));
                std::fs::write(&out_path, slice).expect("failed to write PNG");
                println!("wrote: {}", out_path.display());
            }
        }

        println!("done — check /tmp/icon_overlay_*.png");
        std::process::exit(0);
    });
}
