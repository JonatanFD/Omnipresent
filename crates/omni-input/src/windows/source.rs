//! Capture: low-level keyboard and mouse hooks on a dedicated message-loop
//! thread.
//!
//! `WH_KEYBOARD_LL` and `WH_MOUSE_LL` deliver every keyboard and mouse event to
//! our callbacks before the rest of the system sees them. Each callback
//! translates the event to the protocol vocabulary and queues it for `poll`.
//! While suppressed, the callback also *swallows* the event (returns non-zero
//! instead of chaining) so the local desktop never acts on it — that is what
//! keeps input from landing on both machines while a remote session is active.
//!
//! Both of those jobs are about this machine's *own* keyboard and mouse, so an
//! event Windows flags as injected is left entirely alone: not captured (or the
//! daemon would send its own output back out) and never swallowed. Swallowing it
//! used to break control in the other direction — a machine that had crossed
//! onto a peer stayed suppressed, and while it was, the clicks and keystrokes a
//! peer sent *to* it were eaten by this hook before any application saw them.
//! The cursor still moved, because `SetCursorPos` repositions the pointer
//! whether or not the resulting message survives, so the machine looked alive
//! while nothing it was told to do happened. macOS never had the fault: its tap
//! checks for its own marker before it decides to drop anything.
//!
//! Low-level hooks require a message loop on the thread that installed them, so
//! the hooks live on their own thread, exactly like the macOS run-loop thread.
//! Because the hook callbacks are bare C function pointers that cannot carry
//! state, the small amount of shared state (the event channel, the suppression
//! flag, the held modifiers, the last cursor point) lives in module statics;
//! only one source may exist at a time, which the daemon guarantees.

use super::{WindowsInputError, keymap};
use crate::port::InputSource;
use crate::scroll::Scaled;
use omni_protocol::InputEvent;
use omni_protocol::input::{
    Action, MILLILINES_PER_LINE, Modifiers, MouseButton, MouseDelta, ScrollDelta,
};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU32, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, KillTimer,
    LLKHF_EXTENDED, LLKHF_INJECTED, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, PostThreadMessageW,
    SetCursorPos, SetTimer, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WHEEL_DELTA, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUIT,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WM_XBUTTONDOWN,
    WM_XBUTTONUP, XBUTTON1,
};

use super::{DEFAULT_LINES_PER_NOTCH, screen_center};

/// How often the low-level hooks are torn down and reinstalled, to recover from
/// Windows silently removing one. Short enough that a user notices at most a
/// moment of dead input, long enough to cost nothing.
const HOOK_REARM_INTERVAL_MS: u32 = 5_000;

// Held-modifier bit positions, matching the `Modifiers` constants.
const MOD_SHIFT: u8 = 1 << 0;
const MOD_CTRL: u8 = 1 << 1;
const MOD_ALT: u8 = 1 << 2;
const MOD_META: u8 = 1 << 3;

// Shared state the hook callbacks reach. Single-instance, guarded by INSTALLED.
static INSTALLED: AtomicBool = AtomicBool::new(false);
static EVENT_TX: Mutex<Option<mpsc::Sender<InputEvent>>> = Mutex::new(None);
static SUPPRESSED: AtomicBool = AtomicBool::new(false);
static MODIFIERS: AtomicU8 = AtomicU8::new(0);
static LAST_X: AtomicI32 = AtomicI32::new(0);
static LAST_Y: AtomicI32 = AtomicI32::new(0);
static HAVE_LAST: AtomicBool = AtomicBool::new(false);
// This machine's own scrolling speed, cached from its Windows settings, and what
// each axis has turned so far but not yet reported (see `wheel_millilines`).
static WHEEL_CHARS_PER_NOTCH: AtomicI32 = AtomicI32::new(DEFAULT_LINES_PER_NOTCH);
static WHEEL_LINES_PER_NOTCH: AtomicI32 = AtomicI32::new(DEFAULT_LINES_PER_NOTCH);
static WHEEL_X: Mutex<Scaled> = Mutex::new(Scaled::new());
static WHEEL_Y: Mutex<Scaled> = Mutex::new(Scaled::new());

/// Captures local keyboard and mouse input. The production `InputSource`.
pub struct WindowsSource {
    events: mpsc::Receiver<InputEvent>,
    /// The hook thread's id, so `Drop` can ask it to quit its message loop.
    thread_id: std::sync::Arc<AtomicU32>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for WindowsSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsSource").finish_non_exhaustive()
    }
}

impl WindowsSource {
    /// Installs the low-level hooks on a dedicated thread. Fails if a source is
    /// already running or the hooks cannot be installed.
    pub fn new() -> Result<Self, WindowsInputError> {
        if INSTALLED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(WindowsInputError::AlreadyRunning);
        }

        let (event_tx, events) = mpsc::channel::<InputEvent>();
        *EVENT_TX.lock().expect("event channel lock") = Some(event_tx);
        SUPPRESSED.store(false, Ordering::Relaxed);
        MODIFIERS.store(0, Ordering::Relaxed);
        HAVE_LAST.store(false, Ordering::Relaxed);
        refresh_wheel_settings();
        // Whatever a previous source had turned but not yet reported is stale.
        for axis in [&WHEEL_X, &WHEEL_Y] {
            if let Ok(mut carry) = axis.lock() {
                *carry = Scaled::new();
            }
        }

        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), WindowsInputError>>();
        let thread_id = std::sync::Arc::new(AtomicU32::new(0));
        let thread_id_out = thread_id.clone();
        let thread = std::thread::Builder::new()
            .name("omni-input-hook".into())
            .spawn(move || run_hooks(ready_tx, thread_id_out))
            .map_err(|_| WindowsInputError::HookInstall)?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                events,
                thread_id,
                thread: Some(thread),
            }),
            _ => {
                let _ = thread.join();
                Self::release();
                Err(WindowsInputError::HookInstall)
            }
        }
    }

    /// Clears the shared state so a future source can install cleanly.
    fn release() {
        *EVENT_TX.lock().expect("event channel lock") = None;
        INSTALLED.store(false, Ordering::Release);
    }
}

impl InputSource for WindowsSource {
    type Error = WindowsInputError;

    fn poll(&mut self) -> Result<Option<InputEvent>, Self::Error> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(WindowsInputError::CaptureStopped),
        }
    }

    fn set_suppressed(&mut self, suppressed: bool) {
        SUPPRESSED.store(suppressed, Ordering::Relaxed);
        if suppressed {
            // Park the cursor at the screen centre and anchor relative motion
            // there, so deltas keep flowing instead of stalling at a screen
            // edge while we control the remote machine. The cursor is left
            // parked rather than hidden: hiding it would mean `SetSystemCursor`,
            // which swaps the cursor for the *whole OS* and persists after this
            // process exits — a crash while suppressed would leave every app
            // without a cursor until reboot, which is not worth the cosmetic win.
            let (cx, cy) = screen_center();
            unsafe { SetCursorPos(cx, cy) };
            LAST_X.store(cx, Ordering::Relaxed);
            LAST_Y.store(cy, Ordering::Relaxed);
            HAVE_LAST.store(true, Ordering::Relaxed);
        } else {
            // Re-anchor on the next real move so the cursor does not jump.
            HAVE_LAST.store(false, Ordering::Relaxed);
        }
    }
}

impl Drop for WindowsSource {
    fn drop(&mut self) {
        let id = self.thread_id.load(Ordering::Acquire);
        if id != 0 {
            // Ask the hook thread's message loop to quit so it can unhook.
            unsafe { PostThreadMessageW(id, WM_QUIT, 0, 0) };
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        Self::release();
    }
}

/// The body of the hook thread: install both hooks, publish readiness and this
/// thread's id, then pump messages until asked to quit.
///
/// A timer re-arms the hooks periodically. Windows silently removes a low-level
/// hook whose callback took longer than `LowLevelHooksTimeout` and never says so:
/// the callbacks simply stop being called, `poll` keeps answering "nothing right
/// now", and `omni status` goes on claiming capture is running. Re-installing on
/// a timer bounds how long that can last, and if re-installing fails the thread
/// drops the event channel so `poll` reports the capture as stopped instead of
/// looking idle. (macOS gets an explicit `TapDisabledByTimeout` event and
/// re-enables the tap from the callback.)
fn run_hooks(
    ready: mpsc::Sender<Result<(), WindowsInputError>>,
    thread_id_out: std::sync::Arc<AtomicU32>,
) {
    unsafe {
        let Some(mut hooks) = Hooks::install() else {
            let _ = ready.send(Err(WindowsInputError::HookInstall));
            return;
        };

        thread_id_out.store(
            windows_sys::Win32::System::Threading::GetCurrentThreadId(),
            Ordering::Release,
        );
        let _ = ready.send(Ok(()));

        // A thread timer: with no window it posts WM_TIMER straight to this
        // thread's queue, which the loop below picks up.
        let timer = SetTimer(std::ptr::null_mut(), 0, HOOK_REARM_INTERVAL_MS, None);

        let mut msg = MSG {
            hwnd: std::ptr::null_mut(),
            message: 0,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: POINT { x: 0, y: 0 },
        };
        // GetMessageW returns 0 on WM_QUIT (posted by Drop), -1 on error.
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_TIMER {
                // The same tick picks up a scrolling-speed change made in
                // Windows' settings, so it applies without a restart.
                refresh_wheel_settings();
                if hooks.rearm() {
                    continue;
                }
                // Capture is over and cannot be recovered. Dropping the sender
                // makes the next `poll` report it, so the daemon stops
                // advertising a capture that is not happening.
                *EVENT_TX.lock().expect("event channel lock") = None;
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if timer != 0 {
            KillTimer(std::ptr::null_mut(), timer);
        }
        hooks.remove();
    }
}

/// The pair of installed low-level hooks, so installing and removing them both
/// happens in one place.
struct Hooks {
    keyboard: HHOOK,
    mouse: HHOOK,
}

impl Hooks {
    /// Installs both hooks, or `None` if either could not be installed (in which
    /// case neither is left behind).
    unsafe fn install() -> Option<Self> {
        unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0);
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0);
            if keyboard.is_null() || mouse.is_null() {
                if !keyboard.is_null() {
                    UnhookWindowsHookEx(keyboard);
                }
                if !mouse.is_null() {
                    UnhookWindowsHookEx(mouse);
                }
                return None;
            }
            Some(Self { keyboard, mouse })
        }
    }

    /// Removes and reinstalls both hooks. Returns whether capture is still in
    /// place afterwards; on failure the hooks are gone and the caller must give
    /// up.
    unsafe fn rearm(&mut self) -> bool {
        unsafe {
            self.remove();
            match Self::install() {
                Some(fresh) => {
                    *self = fresh;
                    true
                }
                None => false,
            }
        }
    }

    unsafe fn remove(&mut self) {
        unsafe {
            if !self.keyboard.is_null() {
                UnhookWindowsHookEx(self.keyboard);
                self.keyboard = std::ptr::null_mut();
            }
            if !self.mouse.is_null() {
                UnhookWindowsHookEx(self.mouse);
                self.mouse = std::ptr::null_mut();
            }
        }
    }
}

/// Queues a converted event, ignoring failures (the source may be shutting
/// down).
fn emit(event: InputEvent) {
    if let Ok(guard) = EVENT_TX.lock()
        && let Some(tx) = guard.as_ref()
    {
        let _ = tx.send(event);
    }
}

/// The current held modifiers, as the protocol type.
fn held_modifiers() -> Modifiers {
    let bits = MODIFIERS.load(Ordering::Relaxed);
    let mut result = Modifiers::NONE;
    if bits & MOD_SHIFT != 0 {
        result = result.with(Modifiers::SHIFT);
    }
    if bits & MOD_CTRL != 0 {
        result = result.with(Modifiers::CONTROL);
    }
    if bits & MOD_ALT != 0 {
        result = result.with(Modifiers::ALT);
    }
    if bits & MOD_META != 0 {
        result = result.with(Modifiers::META);
    }
    result
}

/// The held-modifier bit a virtual key controls, if it is a modifier key.
fn modifier_bit(vk: u32) -> Option<u8> {
    match vk {
        0xA0 | 0xA1 | 0x10 => Some(MOD_SHIFT), // L/R/generic Shift
        0xA2 | 0xA3 | 0x11 => Some(MOD_CTRL),  // L/R/generic Control
        0xA4 | 0xA5 | 0x12 => Some(MOD_ALT),   // L/R/generic Alt (Menu)
        0x5B | 0x5C => Some(MOD_META),         // L/R Windows
        _ => None,
    }
}

/// The low-level keyboard hook: convert and queue every key, swallow it while
/// suppressed.
unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 {
        return unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
    }
    let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
    // A synthesized key is not this machine's keyboard, so neither half of the
    // hook's job applies to it: it is not captured, and it is never swallowed.
    // See the note on `is_local_input`.
    if info.flags & LLKHF_INJECTED != 0 {
        return unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
    }
    let message = wparam as u32;
    let action = match message {
        WM_KEYDOWN | WM_SYSKEYDOWN => Action::Press,
        WM_KEYUP | WM_SYSKEYUP => Action::Release,
        _ => return unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) },
    };
    let vk = info.vkCode;
    if let Some(bit) = modifier_bit(vk) {
        match action {
            Action::Press => MODIFIERS.fetch_or(bit, Ordering::Relaxed),
            Action::Release => MODIFIERS.fetch_and(!bit, Ordering::Relaxed),
        };
    }
    // The extended bit is what tells the keypad's Enter from the main one.
    let extended = info.flags & LLKHF_EXTENDED != 0;
    if let Some(hid) = keymap::hid_from_vk(vk as u16, extended) {
        emit(InputEvent::Key {
            code: hid,
            action,
            modifiers: held_modifiers(),
        });
    }
    if SUPPRESSED.load(Ordering::Relaxed) {
        return 1; // swallow: do not pass to the local desktop
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// The low-level mouse hook: convert motion (as relative deltas), buttons, and
/// wheel; swallow everything while suppressed.
unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 {
        return unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
    }
    let info = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
    // Our own SendInput, the re-centring SetCursorPos, and a peer driving this
    // machine all arrive flagged this way — see `is_local_input`.
    if info.flags & LLMHF_INJECTED != 0 {
        return unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
    }
    convert_mouse(wparam as u32, info);
    if SUPPRESSED.load(Ordering::Relaxed) {
        return 1; // swallow
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// Translates one mouse event and queues it.
fn convert_mouse(message: u32, info: &MSLLHOOKSTRUCT) {
    match message {
        WM_MOUSEMOVE => {
            let (x, y) = (info.pt.x, info.pt.y);
            if !HAVE_LAST.swap(true, Ordering::Relaxed) {
                LAST_X.store(x, Ordering::Relaxed);
                LAST_Y.store(y, Ordering::Relaxed);
                return; // first sample: anchor only, no delta
            }
            let dx = x - LAST_X.load(Ordering::Relaxed);
            let dy = y - LAST_Y.load(Ordering::Relaxed);
            if dx != 0 || dy != 0 {
                emit(InputEvent::Motion(MouseDelta::new(dx, dy)));
            }
            if SUPPRESSED.load(Ordering::Relaxed) {
                // Keep the cursor parked at centre so deltas never stall.
                let (cx, cy) = screen_center();
                unsafe { SetCursorPos(cx, cy) };
                LAST_X.store(cx, Ordering::Relaxed);
                LAST_Y.store(cy, Ordering::Relaxed);
            } else {
                LAST_X.store(x, Ordering::Relaxed);
                LAST_Y.store(y, Ordering::Relaxed);
            }
        }
        WM_LBUTTONDOWN => emit_button(MouseButton::Left, Action::Press),
        WM_LBUTTONUP => emit_button(MouseButton::Left, Action::Release),
        WM_RBUTTONDOWN => emit_button(MouseButton::Right, Action::Press),
        WM_RBUTTONUP => emit_button(MouseButton::Right, Action::Release),
        WM_MBUTTONDOWN => emit_button(MouseButton::Middle, Action::Press),
        WM_MBUTTONUP => emit_button(MouseButton::Middle, Action::Release),
        WM_XBUTTONDOWN => emit_button(xbutton(info), Action::Press),
        WM_XBUTTONUP => emit_button(xbutton(info), Action::Release),
        WM_MOUSEWHEEL => {
            let millilines = wheel_millilines(&WHEEL_Y, info, wheel_lines_per_notch());
            if millilines != 0 {
                emit(InputEvent::Scroll(ScrollDelta::new(0, millilines)));
            }
        }
        WM_MOUSEHWHEEL => {
            let millilines = wheel_millilines(&WHEEL_X, info, wheel_chars_per_notch());
            if millilines != 0 {
                emit(InputEvent::Scroll(ScrollDelta::new(millilines, 0)));
            }
        }
        _ => {}
    }
}

/// How far one wheel event turned, in the protocol's milli-lines.
///
/// Two things decide the answer, and leaving either out is what made scrolling
/// feel wrong between machines:
///
/// - **This machine's setting.** `amount_per_notch` is the "lines to scroll per
///   notch" its owner chose, so a notch reports the distance *this* machine
///   would have scrolled. That is the speed the person turning the wheel expects,
///   and the peer receiving it needs the real distance rather than a bare notch
///   count.
/// - **Fractions of a notch.** A precision touchpad or a high-resolution wheel
///   reports far less than a whole notch at a time. Dividing by `WHEEL_DELTA`
///   first floored every one of those to zero, so scrolling with them did
///   nothing at all; the amount is scaled before it is divided, and whatever
///   does not divide evenly is carried into the next event.
fn wheel_millilines(carry: &Mutex<Scaled>, info: &MSLLHOOKSTRUCT, amount_per_notch: i32) -> i32 {
    let raw = (info.mouseData >> 16) as i16 as i32;
    let per_notch = amount_per_notch.saturating_mul(MILLILINES_PER_LINE);
    match carry.lock() {
        Ok(mut carry) => carry.take(raw, per_notch, WHEEL_DELTA as i32),
        // The lock is only ever held for this arithmetic, so it can only be
        // poisoned by a panic that has already brought the process down. Report
        // no movement rather than adding a second failure to the first.
        Err(_) => 0,
    }
}

fn emit_button(button: MouseButton, action: Action) {
    emit(InputEvent::Button { button, action });
}

/// Which extended mouse button an X-button event refers to.
fn xbutton(info: &MSLLHOOKSTRUCT) -> MouseButton {
    let which = (info.mouseData >> 16) as u16;
    if which == XBUTTON1 {
        MouseButton::Back
    } else {
        MouseButton::Forward
    }
}

/// Re-reads this machine's wheel settings into the cache the hook callbacks use.
///
/// Called when the hooks are installed and again on the re-arm timer, so a
/// change made in Windows' own settings takes effect without a restart. The
/// callbacks read the cache rather than asking Windows on every wheel event:
/// Windows silently removes a low-level hook whose callback is slow, and
/// scrolling is the one event that arrives in long bursts.
fn refresh_wheel_settings() {
    let (chars, lines) = super::wheel_scroll_amounts();
    WHEEL_CHARS_PER_NOTCH.store(chars, Ordering::Relaxed);
    WHEEL_LINES_PER_NOTCH.store(lines, Ordering::Relaxed);
}

/// How many characters one notch scrolls sideways, as this machine is set up.
fn wheel_chars_per_notch() -> i32 {
    WHEEL_CHARS_PER_NOTCH.load(Ordering::Relaxed)
}

/// How many lines one notch scrolls, as this machine is set up.
fn wheel_lines_per_notch() -> i32 {
    WHEEL_LINES_PER_NOTCH.load(Ordering::Relaxed)
}
