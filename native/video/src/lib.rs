//! Rust-owned stream state with a minimal dynamically loaded Qt Quick adapter.
mod receiver;

use receiver::VideoReceiver;

#[allow(unsafe_code)] // cxx generates the checked Rust/C++ FFI boundary.
#[cxx::bridge(namespace = "wisp_video")]
mod ffi {
    #[derive(Default)]
    struct Frame {
        changed: bool,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
        error: String,
    }
    struct Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    }
    extern "Rust" {
        type VideoReceiver;
        fn new_receiver() -> Box<VideoReceiver>;
        fn start(self: &mut VideoReceiver, path: &str, participant: &str, source: &str);
        fn viewport(self: &VideoReceiver, width: u32, height: u32);
        fn poll(self: &VideoReceiver) -> Frame;
        fn fit_rect(width: u32, height: u32, available_width: f64, available_height: f64) -> Rect;
    }
}

fn new_receiver() -> Box<VideoReceiver> {
    Box::default()
}

fn fit_rect(width: u32, height: u32, available_width: f64, available_height: f64) -> ffi::Rect {
    if width == 0
        || height == 0
        || !available_width.is_finite()
        || !available_height.is_finite()
        || available_width <= 0.0
        || available_height <= 0.0
    {
        return ffi::Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
    }
    let scale = (available_width / f64::from(width)).min(available_height / f64::from(height));
    let w = f64::from(width) * scale;
    let h = f64::from(height) * scale;
    ffi::Rect {
        x: (available_width - w) / 2.0,
        y: (available_height - h) / 2.0,
        width: w,
        height: h,
    }
}

// Rust cdylibs export Rust entry points, not symbols from linked C++ archives.
// Forward only Qt's documented plugin ABI; Qt owns the returned metadata/object.
// Keep this boundary separate from the safe Rust stream implementation.
#[allow(unsafe_code)] // Only these two Qt loader ABI forwarding functions.
mod plugin_exports {
    use std::ffi::c_void;

    #[repr(C)]
    pub struct Metadata {
        data: *const c_void,
        size: usize,
    }
    unsafe extern "C" {
        fn wisp_qt_plugin_instance() -> *mut c_void;
        fn wisp_qt_plugin_query_metadata_v2() -> Metadata;
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn qt_plugin_instance() -> *mut c_void {
        // SAFETY: exact QObject* ABI; Qt's generated factory owns its singleton.
        unsafe { wisp_qt_plugin_instance() }
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn qt_plugin_query_metadata_v2() -> Metadata {
        // SAFETY: repr(C) matches Qt6 QPluginMetaData (const void*, size_t).
        // The returned storage is static and must never be freed by Rust.
        unsafe { wisp_qt_plugin_query_metadata_v2() }
    }

    #[test]
    fn metadata_is_static_and_exported_with_qt_abi() {
        let first = qt_plugin_query_metadata_v2();
        let second = qt_plugin_query_metadata_v2();
        assert!(!first.data.is_null());
        assert!(first.size > 16);
        assert_eq!((first.data, first.size), (second.data, second.size));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aspect_ratio_and_invalid_geometry() {
        let rect = fit_rect(3440, 1440, 860.0, 600.0);
        assert_eq!(
            (rect.width, rect.height, rect.x, rect.y),
            (860.0, 360.0, 0.0, 120.0)
        );
        let portrait = fit_rect(100, 200, 400.0, 200.0);
        assert_eq!((portrait.width, portrait.x), (100.0, 150.0));
        assert_eq!(fit_rect(0, 2, 100.0, 100.0).width, 0.0);
        assert_eq!(fit_rect(2, 2, f64::NAN, 100.0).width, 0.0);
    }
}
