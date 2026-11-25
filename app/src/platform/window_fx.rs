//! Native Windows open / close / minimize transitions.
//!
//! egui intentionally has no native-window animation API, so this small
//! Windows-only layer uses `AnimateWindow`. Launch starts hidden and fades in;
//! closing fades out before egui exits; a narrow window-proc subclass catches
//! the real title-bar minimize command and fades before minimizing. Restore
//! fades back in. Every action has a normal non-animated Win32 fallback.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AnimateWindow, CallWindowProcW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW,
    SetLayeredWindowAttributes, SetWindowLongPtrW, ShowWindow, AW_ACTIVATE, AW_BLEND, AW_HIDE,
    GWL_EXSTYLE, GWLP_WNDPROC, LWA_ALPHA, SC_CLOSE, SC_MINIMIZE, SC_RESTORE, SW_MINIMIZE,
    WM_CLOSE, WM_SYSCOMMAND, WS_EX_LAYERED,
};

const OPEN_DURATION: Duration = Duration::from_millis(240);
const CLOSE_MS: u32 = 180;
const MINIMIZE_MS: u32 = 180;
const RESTORE_MS: u32 = 160;

static PREVIOUS_WNDPROC: AtomicIsize = AtomicIsize::new(0);
static CLOSE_ANIMATED: AtomicBool = AtomicBool::new(false);

/// The native root window and its one-shot launch state.
pub struct WindowEffects {
    hwnd: HWND,
    opened: bool,
    started_at: Option<Instant>,
    original_ex_style: isize,
}

impl WindowEffects {
    /// Borrow eframe's native HWND. Returns `None` on unsupported platforms.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Option<Self> {
        let raw = cc.window_handle().ok()?.as_raw();
        let RawWindowHandle::Win32(handle) = raw else {
            return None;
        };
        let hwnd = handle.hwnd.get() as HWND;
        if hwnd.is_null() {
            return None;
        }

        // eframe force-shows the window after its first painted frame, so an
        // AnimateWindow launch would be too late. Make the window transparent
        // now, before that frame, then ramp opacity in `tick_open`.
        let (original_ex_style, opened) = unsafe {
            let original = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, original | WS_EX_LAYERED as isize);
            let transparent = SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA) != 0;
            install_minimize_hook(hwnd);
            (original, !transparent)
        };
        Some(Self {
            hwnd,
            opened,
            started_at: None,
            original_ex_style,
        })
    }

    /// Drive the launch fade. Called every egui frame until fully opaque.
    pub fn tick_open(&mut self, ctx: &egui::Context) {
        if self.opened {
            return;
        }
        // Start on the first painted frame, not during app initialization, so
        // the full fade is visible even on a slower machine.
        let started_at = *self.started_at.get_or_insert_with(Instant::now);
        let t = (started_at.elapsed().as_secs_f32() / OPEN_DURATION.as_secs_f32()).clamp(0.0, 1.0);
        // Ease out: a quick, smooth appearance without an abrupt final step.
        let eased = 1.0 - (1.0 - t) * (1.0 - t);
        let alpha = (255.0 * eased).round() as u8;
        unsafe {
            SetLayeredWindowAttributes(self.hwnd, 0, alpha, LWA_ALPHA);
            if t >= 1.0 {
                self.opened = true;
                // Fully opaque: return to the normal, non-layered window mode.
                SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, self.original_ex_style);
            } else {
                ctx.request_repaint();
            }
        }
    }
}

/// Install a one-window subclass. The original winit procedure is retained
/// and receives every message except the minimize command we animate.
unsafe fn install_minimize_hook(hwnd: HWND) {
    let previous = unsafe {
        SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            animated_wnd_proc as *const () as usize as isize,
        )
    };
    if previous != 0 {
        PREVIOUS_WNDPROC.store(previous, Ordering::Release);
    }
}

/// Only the command id matters; the low nibble is used internally by Windows.
const fn command_id(wparam: WPARAM) -> u32 {
    (wparam as u32) & 0xFFF0
}

unsafe extern "system" fn animated_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CLOSE {
        animate_close_once(hwnd);
        // A hidden window may receive no more egui frames, so do not merely
        // queue CloseRequested here. Destroying is the normal WM_CLOSE final
        // step and guarantees winit/eframe observe Destroyed and shut down.
        unsafe {
            if DestroyWindow(hwnd) == 0 {
                return forward_message(hwnd, message, wparam, lparam);
            }
        }
        return 0;
    }
    if message == WM_SYSCOMMAND {
        match command_id(wparam) {
            SC_CLOSE => {
                // The original winit procedure will turn this into WM_CLOSE;
                // the guard prevents a second fade when that message arrives.
                animate_close_once(hwnd);
            }
            SC_MINIMIZE => {
                // Hide with a fade, then enter the real minimized state so the
                // taskbar, Alt+Tab and winit all behave exactly as usual.
                unsafe {
                    AnimateWindow(hwnd, MINIMIZE_MS, AW_BLEND | AW_HIDE);
                    ShowWindow(hwnd, SW_MINIMIZE);
                    // AnimateWindow may temporarily make the window layered. Reset
                    // opacity now, while hidden, so restoring is fully opaque.
                    SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
                }
                return 0;
            }
            SC_RESTORE => {
                let result = unsafe { forward_message(hwnd, message, wparam, lparam) };
                unsafe {
                    AnimateWindow(hwnd, RESTORE_MS, AW_BLEND | AW_ACTIVATE);
                }
                return result;
            }
            _ => {}
        }
    }
    unsafe { forward_message(hwnd, message, wparam, lparam) }
}

fn animate_close_once(hwnd: HWND) {
    if !CLOSE_ANIMATED.swap(true, Ordering::AcqRel) {
        unsafe {
            AnimateWindow(hwnd, CLOSE_MS, AW_BLEND | AW_HIDE);
        }
    }
}

unsafe fn forward_message(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let previous = PREVIOUS_WNDPROC.load(Ordering::Acquire);
    unsafe {
        if previous == 0 {
            DefWindowProcW(hwnd, message, wparam, lparam)
        } else {
            CallWindowProcW(
                std::mem::transmute::<isize, windows_sys::Win32::UI::WindowsAndMessaging::WNDPROC>(
                    previous,
                ),
                hwnd,
                message,
                wparam,
                lparam,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syscommand_masks_the_internal_low_nibble() {
        assert_eq!(command_id(SC_MINIMIZE as usize), SC_MINIMIZE);
        assert_eq!(command_id((SC_MINIMIZE | 0x000F) as usize), SC_MINIMIZE);
        assert_eq!(command_id(SC_RESTORE as usize), SC_RESTORE);
    }
}
