//! Connected displays, for putting the audience view on the projector while
//! the presenter view stays on the laptop. eframe doesn't expose monitors, so
//! this asks the OS directly.
//!
//! Rectangles are in the OS's desktop coordinates: physical pixels on Windows,
//! points on macOS. [`Display::center_in_points`] converts for window placement.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Display {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub primary: bool,
}

impl Display {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// The display's center in egui points, given the main window's native
    /// pixels-per-point (Windows reports physical pixels; macOS already uses points).
    pub fn center_in_points(&self, native_ppp: f32) -> (f32, f32) {
        let k = if cfg!(windows) { native_ppp.max(0.1) as f64 } else { 1.0 };
        (((self.x + self.width / 2.0) / k) as f32, ((self.y + self.height / 2.0) / k) as f32)
    }
}

/// The display that should show the audience view: one that doesn't contain
/// the point (the main window's center, in desktop units), preferring the
/// largest. `None` with a single display.
pub fn audience_display(displays: &[Display], main_window_center: (f64, f64)) -> Option<Display> {
    let (cx, cy) = main_window_center;
    let current = displays.iter().position(|d| d.contains(cx, cy)).or_else(|| displays.iter().position(|d| d.primary));
    displays
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != current)
        .max_by(|(_, a), (_, b)| (a.width * a.height).total_cmp(&(b.width * b.height)))
        .map(|(_, d)| *d)
}

#[cfg(windows)]
pub fn list() -> Vec<Display> {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumDisplayMonitors(hdc: *mut c_void, clip: *const Rect, callback: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut Rect, isize) -> i32, data: isize) -> i32;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
    }
    unsafe extern "system" fn collect(monitor: *mut c_void, _hdc: *mut c_void, _rect: *mut Rect, data: isize) -> i32 {
        // SAFETY: `data` is the `&mut Vec<Display>` passed below, alive for the whole enumeration.
        let out = unsafe { &mut *(data as *mut Vec<Display>) };
        let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
        // SAFETY: `monitor` comes from the OS and `info` is a correctly sized MONITORINFO.
        if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
            let r = &info.monitor;
            out.push(Display {
                x: r.left as f64,
                y: r.top as f64,
                width: (r.right - r.left) as f64,
                height: (r.bottom - r.top) as f64,
                primary: info.flags & 1 != 0,
            });
        }
        1
    }
    let mut out: Vec<Display> = Vec::new();
    // SAFETY: the callback only touches `out` through the pointer we pass, during this call.
    unsafe {
        EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), collect, &mut out as *mut Vec<Display> as isize);
    }
    out
}

#[cfg(target_os = "macos")]
pub fn list() -> Vec<Display> {
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct CGSize {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
        fn CGDisplayBounds(display: u32) -> CGRect;
        fn CGMainDisplayID() -> u32;
    }
    let mut ids = [0u32; 16];
    let mut count = 0u32;
    // SAFETY: the buffer holds 16 ids and CoreGraphics writes at most `max` of them.
    if unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut count) } != 0 {
        return Vec::new();
    }
    // SAFETY: plain value-returning CoreGraphics calls.
    let main = unsafe { CGMainDisplayID() };
    ids[..count as usize]
        .iter()
        .map(|&id| {
            // Global display coordinates: points, origin at the main display's top-left.
            let b = unsafe { CGDisplayBounds(id) };
            Display { x: b.origin.x, y: b.origin.y, width: b.size.width, height: b.size.height, primary: id == main }
        })
        .collect()
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn list() -> Vec<Display> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(x: f64, w: f64, primary: bool) -> Display {
        Display { x, y: 0.0, width: w, height: w * 9.0 / 16.0, primary }
    }

    #[test]
    fn picks_the_other_display() {
        let laptop = d(0.0, 1920.0, true);
        let projector = d(1920.0, 1280.0, false);
        assert_eq!(audience_display(&[laptop, projector], (900.0, 500.0)), Some(projector));
        // The editor was moved to the projector: the laptop gets the show.
        assert_eq!(audience_display(&[laptop, projector], (2500.0, 300.0)), Some(laptop));
        assert_eq!(audience_display(&[laptop], (900.0, 500.0)), None);
        // Three displays: the largest other one.
        let tv = d(-3840.0, 3840.0, false);
        assert_eq!(audience_display(&[laptop, projector, tv], (900.0, 500.0)), Some(tv));
    }

    #[test]
    fn this_machine_reports_its_displays() {
        let all = list();
        if cfg!(any(windows, target_os = "macos")) {
            assert!(!all.is_empty(), "at least one display");
            assert!(all.iter().filter(|d| d.primary).count() <= 1);
            assert!(all.iter().all(|d| d.width > 0.0 && d.height > 0.0));
        }
    }
}
