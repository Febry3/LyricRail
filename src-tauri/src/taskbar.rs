use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use tauri::{PhysicalPosition, PhysicalSize, Position, WebviewWindow, WindowEvent};
use windows_sys::Win32::UI::Shell::{
    SHAppBarMessage, ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_TOP, ABM_GETTASKBARPOS, APPBARDATA,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IsWindowVisible, SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE,
};

const COMPACT_WIDTH: u32 = 560;
pub const MIN_COMPACT_WIDTH: u32 = 360;
pub const MAX_COMPACT_WIDTH: u32 = 720;
const COMPACT_HEIGHT: u32 = 52;
const EXPANDED_HEIGHT: u32 = 178;
const TASKBAR_INSET: i32 = 12;
const TOPMOST_KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(2);

pub fn position_compact_window(window: &WebviewWindow) -> Result<(), String> {
    position_compact_window_with_width(window, COMPACT_WIDTH)
}

pub fn position_compact_window_with_width(
    window: &WebviewWindow,
    width: u32,
) -> Result<(), String> {
    position_window(window, COMPACT_HEIGHT, true, normalize_compact_width(width))
}

pub fn position_expanded_window(window: &WebviewWindow) -> Result<(), String> {
    position_window(window, EXPANDED_HEIGHT, false, COMPACT_WIDTH)
}

pub fn install_window_persistence(window: WebviewWindow) {
    let running = Arc::new(AtomicBool::new(true));
    let event_running = Arc::clone(&running);
    let event_window = window.clone();

    window.on_window_event(move |event| match event {
        WindowEvent::Destroyed => {
            event_running.store(false, Ordering::Relaxed);
        }
        WindowEvent::Focused(false) | WindowEvent::Resized(_) | WindowEvent::Moved(_) => {
            if let Err(error) = reassert_visible_topmost(&event_window) {
                tracing::debug!(%error, "could not reassert taskbar overlay from window event");
            }
        }
        _ => {}
    });

    if let Err(error) = thread::Builder::new()
        .name("taskbar-overlay-topmost".into())
        .spawn(move || {
            while running.load(Ordering::Relaxed) {
                thread::sleep(TOPMOST_KEEP_ALIVE_INTERVAL);

                if let Err(error) = reassert_visible_topmost(&window) {
                    tracing::debug!(%error, "stopping taskbar overlay topmost keep-alive");
                    break;
                }
            }
        })
    {
        tracing::warn!(%error, "could not start taskbar overlay topmost keep-alive");
    }
}

pub fn reassert_visible_topmost(window: &WebviewWindow) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as _;

    unsafe {
        let visible = IsWindowVisible(hwnd) != 0;
        if visible {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }

        let flags =
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | if visible { SWP_SHOWWINDOW } else { 0 };

        if SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, flags) == 0 {
            return Err("SetWindowPos(HWND_TOPMOST) failed for taskbar overlay".into());
        }
    }

    Ok(())
}

pub fn taskbar_is_visible() -> bool {
    let Ok(taskbar) = taskbar_bounds() else {
        return false;
    };

    unsafe { IsWindowVisible(taskbar.window_handle as _) != 0 }
}

fn position_window(
    window: &WebviewWindow,
    height: u32,
    compact: bool,
    width: u32,
) -> Result<(), String> {
    let taskbar = taskbar_bounds()?;
    let (x, y) = placement_for_taskbar(taskbar, height, compact)?;

    window
        .set_size(PhysicalSize::new(width, height))
        .map_err(|error| error.to_string())?;
    window
        .set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(|error| error.to_string())?;
    reassert_visible_topmost(window)
}

#[derive(Clone, Copy)]
struct TaskbarBounds {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    edge: u32,
    window_handle: isize,
}

fn placement_for_taskbar(
    taskbar: TaskbarBounds,
    height: u32,
    compact: bool,
) -> Result<(i32, i32), String> {
    match taskbar.edge {
        ABE_BOTTOM => Ok((
            taskbar.left + TASKBAR_INSET,
            if compact {
                centered_inside(taskbar.top, taskbar.bottom, height)
            } else {
                taskbar.top - height as i32 - TASKBAR_INSET
            },
        )),
        ABE_TOP => Ok((
            taskbar.left + TASKBAR_INSET,
            if compact {
                centered_inside(taskbar.top, taskbar.bottom, height)
            } else {
                taskbar.bottom + TASKBAR_INSET
            },
        )),
        ABE_LEFT => Ok((taskbar.right + TASKBAR_INSET, taskbar.top + TASKBAR_INSET)),
        ABE_RIGHT => Ok((
            taskbar.left - COMPACT_WIDTH as i32 - TASKBAR_INSET,
            taskbar.top + TASKBAR_INSET,
        )),
        _ => Err(format!("unsupported taskbar edge: {}", taskbar.edge)),
    }
}

fn centered_inside(start: i32, end: i32, size: u32) -> i32 {
    start + ((end - start - size as i32).max(0) / 2)
}

fn taskbar_bounds() -> Result<TaskbarBounds, String> {
    let mut appbar = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        ..Default::default()
    };

    let found = unsafe { SHAppBarMessage(ABM_GETTASKBARPOS, &mut appbar) } != 0;
    if !found {
        return Err("Windows did not return the taskbar bounds".into());
    }

    Ok(TaskbarBounds {
        left: appbar.rc.left,
        top: appbar.rc.top,
        right: appbar.rc.right,
        bottom: appbar.rc.bottom,
        edge: appbar.uEdge,
        window_handle: appbar.hWnd as isize,
    })
}

pub fn normalize_compact_width(width: u32) -> u32 {
    width.clamp(MIN_COMPACT_WIDTH, MAX_COMPACT_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_compact_width, placement_for_taskbar, TaskbarBounds, COMPACT_HEIGHT,
        COMPACT_WIDTH, EXPANDED_HEIGHT, MAX_COMPACT_WIDTH, MIN_COMPACT_WIDTH, TASKBAR_INSET,
    };
    use windows_sys::Win32::UI::Shell::{ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_TOP};

    #[test]
    fn compact_overlay_is_centered_inside_bottom_taskbar_band() {
        let taskbar = TaskbarBounds {
            left: 0,
            top: 1020,
            right: 1920,
            bottom: 1080,
            edge: ABE_BOTTOM,
            window_handle: 0,
        };

        let placement = placement_for_taskbar(taskbar, COMPACT_HEIGHT, true).unwrap();

        assert_eq!(placement, (TASKBAR_INSET, 1024));
        assert!(placement.1 >= taskbar.top);
        assert!(placement.1 + COMPACT_HEIGHT as i32 <= taskbar.bottom);
    }

    #[test]
    fn compact_overlay_clamps_to_bottom_taskbar_when_band_is_shorter() {
        let taskbar = TaskbarBounds {
            left: 0,
            top: 1040,
            right: 1920,
            bottom: 1080,
            edge: ABE_BOTTOM,
            window_handle: 0,
        };

        let placement = placement_for_taskbar(taskbar, COMPACT_HEIGHT, true).unwrap();

        assert_eq!(placement, (TASKBAR_INSET, taskbar.top));
    }

    #[test]
    fn expanded_overlay_sits_below_top_taskbar_band() {
        let taskbar = TaskbarBounds {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 40,
            edge: ABE_TOP,
            window_handle: 0,
        };

        let placement = placement_for_taskbar(taskbar, EXPANDED_HEIGHT, false).unwrap();

        assert_eq!(placement, (TASKBAR_INSET, 40 + TASKBAR_INSET));
    }

    #[test]
    fn compact_overlay_sits_beside_left_taskbar_band() {
        let taskbar = TaskbarBounds {
            left: 0,
            top: 0,
            right: 56,
            bottom: 1080,
            edge: ABE_LEFT,
            window_handle: 0,
        };

        let placement = placement_for_taskbar(taskbar, COMPACT_HEIGHT, true).unwrap();

        assert_eq!(placement, (56 + TASKBAR_INSET, TASKBAR_INSET));
    }

    #[test]
    fn compact_overlay_aligns_to_right_taskbar_inner_edge() {
        let taskbar = TaskbarBounds {
            left: 1864,
            top: 0,
            right: 1920,
            bottom: 1080,
            edge: ABE_RIGHT,
            window_handle: 0,
        };

        let placement = placement_for_taskbar(taskbar, COMPACT_HEIGHT, true).unwrap();

        assert_eq!(
            placement,
            (1864 - COMPACT_WIDTH as i32 - TASKBAR_INSET, TASKBAR_INSET)
        );
    }

    #[test]
    fn compact_width_is_clamped_to_the_supported_range() {
        assert_eq!(normalize_compact_width(280), MIN_COMPACT_WIDTH);
        assert_eq!(normalize_compact_width(560), 560);
        assert_eq!(normalize_compact_width(900), MAX_COMPACT_WIDTH);
    }
}
