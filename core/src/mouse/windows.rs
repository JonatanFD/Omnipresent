use crate::mouse::strategy::MouseStrategy;
use crate::network::{ActionType, PhaseType};
use std::mem::size_of;
use std::thread;
use std::time::Duration;

// Native Microsoft imports for mouse and keyboard
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    MOUSE_EVENT_FLAGS, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT,
    SendInput, VIRTUAL_KEY, VK_LCONTROL, VK_LEFT, VK_LWIN, VK_RIGHT, VK_TAB,
};

const SCROLL_THRESHOLD: f32 = 15.0;
// In Windows, one wheel "tick" equals 120 (WHEEL_DELTA)
const WHEEL_DELTA: i32 = 120;

pub struct WindowsMouseStrategy {
    scroll_accumulator_y: f32,
    scroll_accumulator_x: f32,
}

impl WindowsMouseStrategy {
    pub fn new() -> Self {
        Self {
            scroll_accumulator_y: 0.0,
            scroll_accumulator_x: 0.0,
        }
    }

    /// Simulates a mouse event in Windows
    fn send_mouse_input(dx: i32, dy: i32, mouse_data: u32, flags: MOUSE_EVENT_FLAGS) {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: mouse_data,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            let _ = SendInput(&[input], size_of::<INPUT>() as i32);
        }
    }

    /// Simulates a keyboard event in Windows (for swipe gestures)
    fn send_keyboard_input(vk: VIRTUAL_KEY, key_up: bool) {
        let flags = if key_up {
            KEYEVENTF_KEYUP
        } else {
            KEYBD_EVENT_FLAGS(0)
        };
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            let _ = SendInput(&[input], size_of::<INPUT>() as i32);
        }
    }

    fn handle_click_phase(
        button_down: MOUSE_EVENT_FLAGS,
        button_up: MOUSE_EVENT_FLAGS,
        phase: PhaseType,
    ) {
        match phase {
            PhaseType::Start => Self::send_mouse_input(0, 0, 0, button_down),
            PhaseType::End => Self::send_mouse_input(0, 0, 0, button_up),
            _ => {
                Self::send_mouse_input(0, 0, 0, button_down);
                Self::send_mouse_input(0, 0, 0, button_up);
            }
        }
    }
}

impl MouseStrategy for WindowsMouseStrategy {
    fn move_cursor(&mut self, delta_x: f32, delta_y: f32) {
        let dx = delta_x.round() as i32;
        let dy = delta_y.round() as i32;

        if dx != 0 || dy != 0 {
            Self::send_mouse_input(dx, dy, 0, MOUSEEVENTF_MOVE);
        }
    }

    fn execute_action(&mut self, action: ActionType, phase: PhaseType, dx: f32, dy: f32) {
        match action {
            ActionType::RightClick => {
                Self::handle_click_phase(MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, phase)
            }
            ActionType::LeftClick => {
                Self::handle_click_phase(MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, phase)
            }

            ActionType::VerticalScroll => {
                self.scroll_accumulator_y += dy;
                if self.scroll_accumulator_y.abs() >= SCROLL_THRESHOLD {
                    let scroll_direction = if self.scroll_accumulator_y > 0.0 {
                        WHEEL_DELTA // Scroll up
                    } else {
                        -WHEEL_DELTA // Scroll down
                    };
                    Self::send_mouse_input(0, 0, scroll_direction as u32, MOUSEEVENTF_WHEEL);
                    self.scroll_accumulator_y %= SCROLL_THRESHOLD;
                }
            }

            ActionType::HorizontalScroll => {
                self.scroll_accumulator_x += dx;
                if self.scroll_accumulator_x.abs() >= SCROLL_THRESHOLD {
                    let scroll_direction = if self.scroll_accumulator_x > 0.0 {
                        -WHEEL_DELTA // Scroll left
                    } else {
                        WHEEL_DELTA // Scroll right
                    };
                    Self::send_mouse_input(0, 0, scroll_direction as u32, MOUSEEVENTF_HWHEEL);
                    self.scroll_accumulator_x %= SCROLL_THRESHOLD;
                }
            }

            // Gesture: show Task View (all windows) -> Win + Tab
            ActionType::SwipeUp | ActionType::SwipeDown => {
                Self::send_keyboard_input(VK_LWIN, false);
                Self::send_keyboard_input(VK_TAB, false);
                thread::sleep(Duration::from_millis(20));
                Self::send_keyboard_input(VK_TAB, true);
                Self::send_keyboard_input(VK_LWIN, true);
            }

            // Gesture: switch to the virtual desktop on the right -> Ctrl + Win + Right Arrow
            ActionType::SwipeLeft => {
                Self::send_keyboard_input(VK_LCONTROL, false);
                Self::send_keyboard_input(VK_LWIN, false);
                Self::send_keyboard_input(VK_RIGHT, false);
                thread::sleep(Duration::from_millis(20));
                Self::send_keyboard_input(VK_RIGHT, true);
                Self::send_keyboard_input(VK_LWIN, true);
                Self::send_keyboard_input(VK_LCONTROL, true);
            }

            // Gesture: switch to the virtual desktop on the left -> Ctrl + Win + Left Arrow
            ActionType::SwipeRight => {
                Self::send_keyboard_input(VK_LCONTROL, false);
                Self::send_keyboard_input(VK_LWIN, false);
                Self::send_keyboard_input(VK_LEFT, false);
                thread::sleep(Duration::from_millis(20));
                Self::send_keyboard_input(VK_LEFT, true);
                Self::send_keyboard_input(VK_LWIN, true);
                Self::send_keyboard_input(VK_LCONTROL, true);
            }

            ActionType::NoAction => {
                self.scroll_accumulator_y = 0.0;
                self.scroll_accumulator_x = 0.0;
            }

            // Fallback for any unhandled future enum variants
            _ => {}
        }
    }
}
