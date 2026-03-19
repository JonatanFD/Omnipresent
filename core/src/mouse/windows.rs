// Este archivo solo se compila en Windows, así que podemos usar librerías de Windows con seguridad
use crate::mouse::strategy::MouseStrategy;
use crate::network::{ActionType, PhaseType};

// Importamos las librerías nativas de Microsoft
use std::mem::size_of;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
    MOUSEINPUT, SendInput,
};

pub struct WindowsMouseStrategy;

impl WindowsMouseStrategy {
    pub fn new() -> Self {
        WindowsMouseStrategy
    }

    /// Función auxiliar para enviar comandos crudos a Windows
    fn send_mouse_input(dx: i32, dy: i32, flags: u32) {
        let mut input = INPUT {
            type_: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            // Llamada directa al kernel/subsistema de UI de Windows
            SendInput(&[input], size_of::<INPUT>() as i32);
        }
    }
}

impl MouseStrategy for WindowsMouseStrategy {
    fn move_cursor(&mut self, dx: f32, dy: f32) {
        // Multiplicamos por sensibilidad si es necesario y enviamos el delta
        Self::send_mouse_input(dx as i32, dy as i32, MOUSEEVENTF_MOVE);
    }

    fn execute_action(&mut self, action: ActionType, phase: PhaseType, _dx: f32, _dy: f32) {
        match action {
            ActionType::LeftClick => match phase {
                PhaseType::Start => Self::send_mouse_input(0, 0, MOUSEEVENTF_LEFTDOWN),
                PhaseType::End => Self::send_mouse_input(0, 0, MOUSEEVENTF_LEFTUP),
                _ => {}
            },
            // ... Aquí implementarías RightClick, Scroll, etc.
            _ => {}
        }
    }
}
