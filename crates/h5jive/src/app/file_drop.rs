use std::path::PathBuf;
use winit::{event::WindowEvent, window::Window};

#[derive(Default)]
pub(super) struct FileDropState {
    pub hovering: bool,
    pub pointer_x: Option<f32>,
    consumed: bool,
}

pub(super) enum DropAction {
    None,
    Open(usize, PathBuf),
    UnknownPosition,
}

impl FileDropState {
    pub fn handle_event(&mut self, event: &WindowEvent, window: &Window) -> DropAction {
        match event {
            WindowEvent::HoveredFile(_) => {
                if !self.hovering {
                    self.consumed = false;
                    self.pointer_x = None;
                }
                self.hovering = true;
                self.refresh_pointer(window);
            }
            WindowEvent::CursorMoved { position, .. } if self.hovering => {
                self.pointer_x = Some(position.x as f32 / window.scale_factor() as f32);
            }
            WindowEvent::HoveredFileCancelled => {
                self.hovering = false;
                self.pointer_x = None;
                self.consumed = false;
            }
            WindowEvent::DroppedFile(path) if !self.consumed => {
                self.refresh_pointer(window);
                self.hovering = false;
                self.consumed = true;
                let width = window.inner_size().width as f32 / window.scale_factor() as f32;
                return drop_slot(self.pointer_x, width)
                    .map_or(DropAction::UnknownPosition, |slot| {
                        DropAction::Open(slot, path.clone())
                    });
            }
            _ => {}
        }
        DropAction::None
    }

    pub fn refresh_pointer(&mut self, window: &Window) {
        if let Some(x) = native_pointer_x(window) {
            self.pointer_x = Some(x);
        }
    }
}

pub(super) fn drop_slot(pointer_x: Option<f32>, width: f32) -> Option<usize> {
    let x = pointer_x?;
    (width > 0.0 && x.is_finite() && x >= 0.0 && x < width).then_some(usize::from(x >= width * 0.5))
}

fn native_pointer_x(window: &Window) -> Option<f32> {
    let global_x = global_pointer_x(window)?;
    let left = window.inner_position().ok()?.x as f64;
    let x = (global_x - left) / window.scale_factor();
    x.is_finite().then_some(x as f32)
}

#[cfg(target_os = "windows")]
fn global_pointer_x(_window: &Window) -> Option<f64> {
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetCursorPos(point: *mut Point) -> i32;
    }
    let mut point = Point { x: 0, y: 0 };
    (unsafe { GetCursorPos(&mut point) } != 0).then_some(f64::from(point.x))
}

#[cfg(target_os = "macos")]
fn global_pointer_x(window: &Window) -> Option<f64> {
    use std::ffi::c_void;
    #[repr(C)]
    struct Point {
        x: f64,
        y: f64,
    }
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreate(source: *const c_void) -> *mut c_void;
        fn CGEventGetLocation(event: *const c_void) -> Point;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(object: *const c_void);
    }
    let event = unsafe { CGEventCreate(std::ptr::null()) };
    if event.is_null() {
        return None;
    }
    let point = unsafe { CGEventGetLocation(event) };
    unsafe { CFRelease(event) };
    // Quartz screen coordinates are in points; winit's inner_position is in pixels.
    Some(point.x * window.scale_factor())
}

#[cfg(target_os = "linux")]
fn global_pointer_x(_window: &Window) -> Option<f64> {
    use x11rb::{connection::Connection, protocol::xproto::ConnectionExt};
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots.get(screen)?.root;
    let pointer = connection.query_pointer(root).ok()?.reply().ok()?;
    Some(f64::from(pointer.root_x))
}

#[cfg(test)]
mod tests {
    use super::drop_slot;

    #[test]
    fn drop_target_uses_left_and_right_halves() {
        assert_eq!(drop_slot(Some(0.0), 800.0), Some(0));
        assert_eq!(drop_slot(Some(399.0), 800.0), Some(0));
        assert_eq!(drop_slot(Some(400.0), 800.0), Some(1));
        assert_eq!(drop_slot(Some(799.0), 800.0), Some(1));
        assert_eq!(drop_slot(None, 800.0), None);
        assert_eq!(drop_slot(Some(800.0), 800.0), None);
    }
}
