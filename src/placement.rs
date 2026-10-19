//! Window placement, sizing and focus helpers.

#[cfg(windows)]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(windows)]
use slint::winit_030::WinitWindowAccessor;
#[cfg(windows)]
use slint::winit_030::winit::window::ResizeDirection;
use slint::{LogicalSize, PhysicalPosition};

use crate::windows_integration::{self, PopupAnchor, WorkArea};

pub const POPUP_LOGICAL_WIDTH: f32 = 520.0;
pub const POPUP_LOGICAL_HEIGHT: f32 = 320.0;
const SETTINGS_LOGICAL_WIDTH: f32 = 460.0;
const SETTINGS_LOGICAL_HEIGHT: f32 = 390.0;

pub fn popup_size() -> LogicalSize {
    LogicalSize::new(POPUP_LOGICAL_WIDTH, POPUP_LOGICAL_HEIGHT)
}

/// Places the window's top-left corner at the cursor, then moves it fully onto
/// the work area of the cursor's monitor, like the original popup.
pub fn position_popup(window: &slint::Window, anchor: PopupAnchor) {
    let area = anchor.work_area;
    let (width, height) = physical_size(area, POPUP_LOGICAL_WIDTH, POPUP_LOGICAL_HEIGHT);
    window.set_position(PhysicalPosition {
        x: shift_into(anchor.cursor_x, area.left, area.right, width),
        y: shift_into(anchor.cursor_y, area.top, area.bottom, height),
    });
}

pub fn center_in_work_area(window: &slint::Window, area: WorkArea) {
    let (width, height) = physical_size(area, SETTINGS_LOGICAL_WIDTH, SETTINGS_LOGICAL_HEIGHT);
    window.set_position(PhysicalPosition {
        x: area.left + (area.right - area.left - width).max(0) / 2,
        y: area.top + (area.bottom - area.top - height).max(0) / 2,
    });
}

fn physical_size(area: WorkArea, width: f32, height: f32) -> (i32, i32) {
    let scale_factor = area.scale_factor.max(0.5);
    (
        (width * scale_factor).round() as i32,
        (height * scale_factor).round() as i32,
    )
}

fn shift_into(value: i32, start: i32, end: i32, size: i32) -> i32 {
    if end - start < size {
        start
    } else {
        value.clamp(start, end - size)
    }
}

/// How the popup makes room for an open language list that reaches below the card.
#[derive(Debug, PartialEq)]
pub struct MenuFit {
    /// Physical pixels to move the window up.
    pub shift: i32,
    /// Logical height to add below the card.
    pub extra: f32,
}

/// `needed` is the logical height the list reaches beyond `base_height`. The
/// window grows downward as far as the work area allows and moves up for the rest.
pub fn fit_menu(
    needed: f32,
    base_height: f32,
    window_y: i32,
    area: WorkArea,
    scale_factor: f32,
) -> MenuFit {
    if needed <= 0.0 {
        return MenuFit {
            shift: 0,
            extra: 0.0,
        };
    }
    let room_below = |y: i32| (area.bottom - y) as f32 / scale_factor - base_height;
    let missing = needed - room_below(window_y);
    let shift = if missing > 0.0 {
        ((missing * scale_factor).ceil() as i32)
            .min(window_y - area.top)
            .max(0)
    } else {
        0
    };
    MenuFit {
        shift,
        extra: needed.min(room_below(window_y - shift).max(0.0)),
    }
}

pub fn cursor_is_inside(window: &slint::Window) -> bool {
    let position = window.position();
    let size = window.size();
    windows_integration::cursor_is_in_rect(
        position.x,
        position.y,
        position.x + size.width as i32,
        position.y + size.height as i32,
    )
}

pub fn activate_window(window: &slint::Window) {
    #[cfg(windows)]
    {
        let _ = window.with_winit_window(|window| window.focus_window());
        if let Some(handle) = native_window_handle(window) {
            windows_integration::bring_to_foreground(handle);
        }
    }
    #[cfg(not(windows))]
    let _ = window;
}

pub fn window_has_focus(window: &slint::Window) -> bool {
    #[cfg(windows)]
    {
        window
            .with_winit_window(|window| window.has_focus())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        true
    }
}

pub fn drag_window(window: &slint::Window) {
    #[cfg(windows)]
    let _ = window.with_winit_window(|window| window.drag_window());
    #[cfg(not(windows))]
    let _ = window;
}

pub fn resize_window(window: &slint::Window) {
    #[cfg(windows)]
    let _ =
        window.with_winit_window(|window| window.drag_resize_window(ResizeDirection::SouthEast));
    #[cfg(not(windows))]
    let _ = window;
}

#[cfg(windows)]
pub fn native_window_handle(window: &slint::Window) -> Option<isize> {
    window
        .with_winit_window(|window| {
            let handle = window.window_handle().ok()?.as_raw();
            match handle {
                RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
                _ => None,
            }
        })
        .flatten()
}

#[cfg(not(windows))]
pub fn native_window_handle(_window: &slint::Window) -> Option<isize> {
    Some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: WorkArea = WorkArea {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1000,
        scale_factor: 2.0,
    };

    #[test]
    fn popup_is_shifted_onto_the_work_area() {
        assert_eq!(shift_into(100, 0, 1920, 520), 100);
        assert_eq!(shift_into(1800, 0, 1920, 520), 1400);
        assert_eq!(shift_into(-50, 0, 1920, 520), 0);
        assert_eq!(shift_into(10, 0, 300, 520), 0);
    }

    #[test]
    fn menu_grows_the_window_downward_when_there_is_room() {
        // Window at y=100 with a 320 logical (640 px) base: 130 logical below.
        assert_eq!(
            fit_menu(100.0, 320.0, 100, AREA, 2.0),
            MenuFit {
                shift: 0,
                extra: 100.0
            }
        );
        assert_eq!(
            fit_menu(0.0, 320.0, 100, AREA, 2.0),
            MenuFit {
                shift: 0,
                extra: 0.0
            }
        );
    }

    #[test]
    fn menu_moves_the_window_up_when_the_bottom_is_near() {
        // Only 30 logical below; the remaining 70 logical (140 px) come from moving up.
        assert_eq!(
            fit_menu(100.0, 320.0, 300, AREA, 2.0),
            MenuFit {
                shift: 140,
                extra: 100.0
            }
        );
        // Cannot move above the work area.
        assert_eq!(
            fit_menu(100.0, 320.0, 300, WorkArea { top: 250, ..AREA }, 2.0),
            MenuFit {
                shift: 50,
                extra: 55.0
            }
        );
    }
}
