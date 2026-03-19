use crate::mouse::strategy::MouseStrategy;
use crate::network::{ActionType, PhaseType};

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use std::ffi::c_void;
use std::thread;
use std::time::Duration;

// kVK_* virtual keycodes
const KEY_LEFT: u16 = 123;
const KEY_RIGHT: u16 = 124;
const KEY_DOWN: u16 = 125;
const KEY_UP: u16 = 126;
const KEY_CONTROL: u16 = 59; // kVK_Control

const SCROLL_THRESHOLD: f32 = 1.0;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateScrollWheelEvent(
        source: *const c_void,
        units: u32, // 1 = kCGScrollEventUnitLine
        wheelCount: u32,
        wheel1: i32, // eje vertical
        wheel2: i32, // eje horizontal
    ) -> *mut c_void;

    fn CGEventPost(tapLocation: u32, event: *mut c_void);
    fn CFRelease(cf: *mut c_void);
}

pub struct MacOsMouseStrategy {
    event_source: CGEventSource,
    scroll_accumulator_y: f32,
    scroll_accumulator_x: f32,
    is_left_down: bool,
    is_right_down: bool,
}

unsafe impl Send for MacOsMouseStrategy {}
unsafe impl Sync for MacOsMouseStrategy {}

impl MacOsMouseStrategy {
    pub fn new() -> Self {
        let event_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .expect("FATAL: No se pudo crear CGEventSource");
        Self {
            event_source,
            scroll_accumulator_y: 0.0,
            scroll_accumulator_x: 0.0,
            is_left_down: false,
            is_right_down: false,
        }
    }

    fn current_position(&self) -> CGPoint {
        CGEvent::new(self.event_source.clone())
            .map(|e| e.location())
            .unwrap_or(CGPoint::new(0.0, 0.0))
    }

    fn handle_click_phase(
        &mut self,
        down_type: CGEventType,
        up_type: CGEventType,
        button: CGMouseButton,
        phase: PhaseType,
    ) {
        match button {
            CGMouseButton::Left => self.is_left_down = phase == PhaseType::Start,
            CGMouseButton::Right => self.is_right_down = phase == PhaseType::Start,
            _ => {}
        }

        let pos = self.current_position();
        let src = self.event_source.clone();

        match phase {
            PhaseType::Start => {
                if let Ok(ev) = CGEvent::new_mouse_event(src, down_type, pos, button) {
                    ev.post(CGEventTapLocation::HID);
                }
            }
            PhaseType::End => {
                if let Ok(ev) = CGEvent::new_mouse_event(src, up_type, pos, button) {
                    ev.post(CGEventTapLocation::HID);
                }
            }
            _ => {
                if let Ok(ev) = CGEvent::new_mouse_event(src.clone(), down_type, pos, button) {
                    ev.post(CGEventTapLocation::HID);
                }
                if let Ok(ev) = CGEvent::new_mouse_event(src, up_type, pos, button) {
                    ev.post(CGEventTapLocation::HID);
                }
            }
        }
    }

    /// Secuencia completa: keyDown(CTRL) → keyDown(flecha+flag) → keyUp(flecha+flag) → keyUp(CTRL)
    /// macOS valida el modificador a nivel de window server; sin esta secuencia
    /// los atajos de sistema (espacios, Mission Control) no se disparan.
    fn send_ctrl_arrow(&self, keycode: u16) {
        let ctrl_flag = CGEventFlags::CGEventFlagControl;
        let src = self.event_source.clone();

        // 1. Bajar Control
        if let Ok(ev) = CGEvent::new_keyboard_event(src.clone(), KEY_CONTROL, true) {
            ev.set_flags(ctrl_flag);
            ev.post(CGEventTapLocation::HID);
        }
        thread::sleep(Duration::from_millis(10));

        // 2. Bajar la tecla de dirección con el flag activo
        if let Ok(ev) = CGEvent::new_keyboard_event(src.clone(), keycode, true) {
            ev.set_flags(ctrl_flag);
            ev.post(CGEventTapLocation::HID);
        }
        thread::sleep(Duration::from_millis(50));

        // 3. Soltar la tecla de dirección (Control todavía presionado)
        if let Ok(ev) = CGEvent::new_keyboard_event(src.clone(), keycode, false) {
            ev.set_flags(ctrl_flag);
            ev.post(CGEventTapLocation::HID);
        }
        thread::sleep(Duration::from_millis(10));

        // 4. Soltar Control
        if let Ok(ev) = CGEvent::new_keyboard_event(src, KEY_CONTROL, false) {
            ev.set_flags(CGEventFlags::empty());
            ev.post(CGEventTapLocation::HID);
        }
    }

    fn emit_scroll(&self, lines_y: i32, lines_x: i32) {
        unsafe {
            let ev = CGEventCreateScrollWheelEvent(
                std::ptr::null(),
                1, // kCGScrollEventUnitLine — funciona en todas las apps
                2, // dos ejes
                lines_y,
                lines_x,
            );
            if !ev.is_null() {
                CGEventPost(0, ev); // 0 = kCGHIDEventTap
                CFRelease(ev);
            }
        }
    }
}

impl MouseStrategy for MacOsMouseStrategy {
    fn move_cursor(&mut self, delta_x: f32, delta_y: f32) {
        let pos = self.current_position();
        let new_pos = CGPoint::new(pos.x + delta_x as f64, pos.y + delta_y as f64);

        let (event_type, button) = if self.is_left_down {
            (CGEventType::LeftMouseDragged, CGMouseButton::Left)
        } else if self.is_right_down {
            (CGEventType::RightMouseDragged, CGMouseButton::Right)
        } else {
            (CGEventType::MouseMoved, CGMouseButton::Left)
        };

        if let Ok(ev) =
            CGEvent::new_mouse_event(self.event_source.clone(), event_type, new_pos, button)
        {
            ev.post(CGEventTapLocation::HID);
        }
    }

    fn execute_action(&mut self, action: ActionType, phase: PhaseType, dx: f32, dy: f32) {
        match action {
            ActionType::LeftClick => {
                self.handle_click_phase(
                    CGEventType::LeftMouseDown,
                    CGEventType::LeftMouseUp,
                    CGMouseButton::Left,
                    phase,
                );
            }

            ActionType::RightClick => {
                self.handle_click_phase(
                    CGEventType::RightMouseDown,
                    CGEventType::RightMouseUp,
                    CGMouseButton::Right,
                    phase,
                );
            }

            // Espejo de Linux REL_WHEEL — mismo threshold, mismo patrón %=
            ActionType::VerticalScroll => {
                self.scroll_accumulator_y += dy;
                if self.scroll_accumulator_y.abs() >= SCROLL_THRESHOLD {
                    let direction = if self.scroll_accumulator_y > 0.0 {
                        1
                    } else {
                        -1
                    };
                    self.emit_scroll(direction, 0);
                    self.scroll_accumulator_y %= SCROLL_THRESHOLD;
                }
            }

            // Espejo de Linux REL_HWHEEL — dirección invertida igual que en Linux
            ActionType::HorizontalScroll => {
                self.scroll_accumulator_x += dx;
                if self.scroll_accumulator_x.abs() >= SCROLL_THRESHOLD {
                    let direction = if self.scroll_accumulator_x > 0.0 {
                        -1
                    } else {
                        1
                    };
                    self.emit_scroll(0, direction);
                    self.scroll_accumulator_x %= SCROLL_THRESHOLD;
                }
            }

            // Linux: Meta solo → macOS: Ctrl+Up (Mission Control)
            ActionType::SwipeUp => self.send_ctrl_arrow(KEY_UP),

            // Linux: Meta solo → macOS: Ctrl+Down (App Exposé)
            ActionType::SwipeDown => self.send_ctrl_arrow(KEY_DOWN),

            // Linux: Ctrl+Alt+Right → macOS: Ctrl+Right (siguiente espacio)
            ActionType::SwipeLeft => self.send_ctrl_arrow(KEY_RIGHT),

            // Linux: Ctrl+Alt+Left → macOS: Ctrl+Left (espacio anterior)
            ActionType::SwipeRight => self.send_ctrl_arrow(KEY_LEFT),

            ActionType::NoAction => {
                self.scroll_accumulator_y = 0.0;
                self.scroll_accumulator_x = 0.0;
            }
        }
    }
}
