//! Native Windows open / close / minimize transitions.
//!
//! egui intentionally has no native-window animation API, so this small
//! Windows-only layer fades the real window itself. Launch starts hidden and
//! fades in; closing blends out before egui exits; a narrow window-proc
//! subclass catches the title-bar minimize/restore commands and drives a
//! timer-based opacity ramp before changing the window state. Every action has
//! a normal non-animated Win32 fallback.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::ProcessStatus::EmptyWorkingSet;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AW_BLEND, AW_HIDE, AnimateWindow, CallWindowProcW, DefWindowProcW, DestroyWindow, GWL_EXSTYLE,
    GWLP_WNDPROC, GetWindowLongPtrW, IsIconic, KillTimer, LWA_ALPHA, SC_CLOSE, SC_MINIMIZE,
    SC_RESTORE, SW_MINIMIZE, SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW, ShowWindow,
    WM_CLOSE, WM_SYSCOMMAND, WM_TIMER, WS_EX_LAYERED,
};

const OPEN_DURATION: Duration = Duration::from_millis(240);
const CLOSE_MS: u32 = 180;
const MINIMIZE_DURATION: Duration = Duration::from_millis(200);
const RESTORE_DURATION: Duration = Duration::from_millis(170);
const TRANSITION_TIMER_ID: usize = 0x534B_4E4A; // "SKNJ"

static PREVIOUS_WNDPROC: AtomicIsize = AtomicIsize::new(0);
static CLOSE_ANIMATED: AtomicBool = AtomicBool::new(false);
static TRIM_ON_MINIMIZE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq)]
enum TransitionKind {
    Idle,
    Minimizing,
    Restoring,
}

struct TransitionState {
    kind: TransitionKind,
    started_at: Option<Instant>,
    original_ex_style: isize,
}

impl TransitionState {
    const fn new() -> Self {
        Self {
            kind: TransitionKind::Idle,
            started_at: None,
            original_ex_style: 0,
        }
    }
}

// The window procedure and its timer both run on the native UI thread.
static TRANSITION: Mutex<TransitionState> = Mutex::new(TransitionState::new());

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

    /// Minimize through the same smooth native fade as the title-bar button,
    /// then trim unused pages for the explicit low-power background mode.
    pub fn minimize_to_background(&self) {
        TRIM_ON_MINIMIZE.store(true, Ordering::Release);
        begin_transition(self.hwnd, TransitionKind::Minimizing);
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
                // Do not call AnimateWindow here: its hide + SW_MINIMIZE
                // sequence caused a visible snap. A native timer now drives a
                // smooth opacity ramp, and only then minimizes the window.
                begin_transition(hwnd, TransitionKind::Minimizing);
                return 0;
            }
            SC_RESTORE => {
                // Only fade when coming back from the minimized state. Windows
                // also sends SC_RESTORE when un-maximizing, where a fade would
                // look out of place.
                let was_minimized = unsafe { IsIconic(hwnd) } != 0;
                let was_minimizing = transition_kind() == TransitionKind::Minimizing;
                if was_minimized {
                    // Set opacity to zero *before* Windows shows the restored
                    // window. Starting the fade afterwards allowed one opaque
                    // frame through, which looked like a blink on taskbar click.
                    begin_transition(hwnd, TransitionKind::Restoring);
                }
                let result = unsafe { forward_message(hwnd, message, wparam, lparam) };
                if !was_minimized && was_minimizing {
                    // The window never actually minimized; cancel the fade so
                    // its timer cannot minimize the window after this restore.
                    cancel_minimize_transition(hwnd);
                }
                return result;
            }
            _ => {}
        }
    } else if message == WM_TIMER && wparam == TRANSITION_TIMER_ID {
        advance_transition(hwnd);
        return 0;
    }
    unsafe { forward_message(hwnd, message, wparam, lparam) }
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn transition_kind() -> TransitionKind {
    TRANSITION
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .kind
}

/// Abort a minimize fade that never reached the hidden state (the user
/// restored almost immediately). Returns the window to full opacity.
fn cancel_minimize_transition(hwnd: HWND) {
    let mut state = TRANSITION.lock().unwrap_or_else(|e| e.into_inner());
    if state.kind != TransitionKind::Minimizing {
        return;
    }
    unsafe {
        KillTimer(hwnd, TRANSITION_TIMER_ID);
        SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, state.original_ex_style);
    }
    state.kind = TransitionKind::Idle;
    state.started_at = None;
    TRIM_ON_MINIMIZE.store(false, Ordering::Release);
}

fn begin_transition(hwnd: HWND, kind: TransitionKind) {
    let mut state = TRANSITION.lock().unwrap_or_else(|e| e.into_inner());
    if state.kind == kind {
        return;
    }
    if state.kind != TransitionKind::Idle {
        // A new command interrupted the opposite fade. Cancel the old timer so
        // it cannot apply the stale end state.
        unsafe {
            KillTimer(hwnd, TRANSITION_TIMER_ID);
        }
        state.kind = TransitionKind::Idle;
        state.started_at = None;
    }

    unsafe {
        // When interrupting a fade, the current style still has WS_EX_LAYERED;
        // strip it so the end of the new transition restores the true style.
        let original = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & !(WS_EX_LAYERED as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, original | WS_EX_LAYERED as isize);
        let alpha = if kind == TransitionKind::Minimizing {
            255
        } else {
            0
        };
        if SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA) == 0 {
            // The normal command still works if layered rendering is denied.
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, original);
            if kind == TransitionKind::Minimizing {
                ShowWindow(hwnd, SW_MINIMIZE);
            }
            return;
        }
        state.original_ex_style = original;
        state.kind = kind;
        state.started_at = Some(Instant::now());
        if SetTimer(hwnd, TRANSITION_TIMER_ID, 16, None) == 0 {
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, original);
            state.kind = TransitionKind::Idle;
            state.started_at = None;
            if kind == TransitionKind::Minimizing {
                ShowWindow(hwnd, SW_MINIMIZE);
            }
        }
    }
}

fn advance_transition(hwnd: HWND) {
    let (kind, t, original_ex_style) = {
        let state = TRANSITION.lock().unwrap_or_else(|e| e.into_inner());
        let Some(started_at) = state.started_at else {
            return;
        };
        let duration = match state.kind {
            TransitionKind::Minimizing => MINIMIZE_DURATION,
            TransitionKind::Restoring => RESTORE_DURATION,
            TransitionKind::Idle => return,
        };
        (
            state.kind,
            (started_at.elapsed().as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0),
            state.original_ex_style,
        )
    };

    let progress = smoothstep(t);
    let alpha = match kind {
        TransitionKind::Minimizing => 255.0 * (1.0 - progress),
        TransitionKind::Restoring => 255.0 * progress,
        TransitionKind::Idle => return,
    };
    unsafe {
        SetLayeredWindowAttributes(hwnd, 0, alpha.round() as u8, LWA_ALPHA);
        if t < 1.0 {
            return;
        }

        KillTimer(hwnd, TRANSITION_TIMER_ID);
        if kind == TransitionKind::Minimizing {
            // Minimize while still transparent. Restore opacity and the normal
            // window style only after it is hidden, preventing any flash.
            ShowWindow(hwnd, SW_MINIMIZE);
            if TRIM_ON_MINIMIZE.swap(false, Ordering::AcqRel) {
                // Only the explicit background mode requests this. It runs
                // after the fade, so normal window interactions never pay the
                // page-fault cost of trimming the working set.
                EmptyWorkingSet(GetCurrentProcess());
            }
        }
        SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, original_ex_style);
    }
    let mut state = TRANSITION.lock().unwrap_or_else(|e| e.into_inner());
    state.kind = TransitionKind::Idle;
    state.started_at = None;
}

fn animate_close_once(hwnd: HWND) {
    if !CLOSE_ANIMATED.swap(true, Ordering::AcqRel) {
        unsafe {
            KillTimer(hwnd, TRANSITION_TIMER_ID);
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

    #[test]
    fn smoothstep_has_soft_endpoints() {
        assert_eq!(smoothstep(0.0), 0.0);
        assert_eq!(smoothstep(1.0), 1.0);
        assert_eq!(smoothstep(0.5), 0.5);
        assert!(smoothstep(0.1) < 0.1);
        assert!(smoothstep(0.9) > 0.9);
        assert_eq!(smoothstep(2.0), 1.0);
    }
}
