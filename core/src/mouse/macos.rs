use crate::mouse::strategy::MouseStrategy;
use crate::network::{ActionType, PhaseType};

// Importamos la librería de alto nivel Enigo (v0.6.1) y su enum Axis
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use std::thread;
use std::time::Duration;

pub struct MacOsMouseStrategy {
    enigo: Enigo,
    scroll_accumulator_y: f32,
    scroll_accumulator_x: f32,
}

impl MacOsMouseStrategy {
    pub fn new() -> Self {
        // Inicializamos Enigo con la configuración por defecto
        let enigo = Enigo::new(&Settings::default()).expect("FATAL: No se pudo inicializar Enigo");

        Self {
            enigo,
            scroll_accumulator_y: 0.0,
            scroll_accumulator_x: 0.0,
        }
    }

    /// Presiona la tecla Control, luego da un toque a la Flecha, y suelta Control
    fn send_shortcut(&mut self, key: Key) {
        let _ = self.enigo.key(Key::Control, Direction::Press);
        thread::sleep(Duration::from_millis(20));
        let _ = self.enigo.key(key, Direction::Click);
        thread::sleep(Duration::from_millis(20));
        let _ = self.enigo.key(Key::Control, Direction::Release);
    }
}

impl MouseStrategy for MacOsMouseStrategy {
    fn move_cursor(&mut self, delta_x: f32, delta_y: f32) {
        // Coordinate::Rel mueve el ratón de forma relativa a donde está ahora.
        // Enigo maneja automáticamente si el botón está presionado para hacer el "Arrastre".
        let _ = self
            .enigo
            .move_mouse(delta_x as i32, delta_y as i32, Coordinate::Rel);
    }

    fn execute_action(&mut self, action: ActionType, phase: PhaseType, dx: f32, dy: f32) {
        match action {
            // Manejo de Clic Izquierdo (Start = Presionar, End = Soltar, Otro = Clic rápido)
            ActionType::LeftClick => match phase {
                PhaseType::Start => {
                    let _ = self.enigo.button(Button::Left, Direction::Press);
                }
                PhaseType::End => {
                    let _ = self.enigo.button(Button::Left, Direction::Release);
                }
                _ => {
                    let _ = self.enigo.button(Button::Left, Direction::Click);
                }
            },

            // Manejo de Clic Derecho
            ActionType::RightClick => match phase {
                PhaseType::Start => {
                    let _ = self.enigo.button(Button::Right, Direction::Press);
                }
                PhaseType::End => {
                    let _ = self.enigo.button(Button::Right, Direction::Release);
                }
                _ => {
                    let _ = self.enigo.button(Button::Right, Direction::Click);
                }
            },

            // Manejo del Scroll (Con la corrección de Axis para Enigo 0.6.1)
            ActionType::VerticalScroll | ActionType::HorizontalScroll => {
                self.scroll_accumulator_y += dy;
                self.scroll_accumulator_x += dx;

                let threshold = 5.0; // Sensibilidad del scroll

                // Evaluamos y ejecutamos el Scroll Vertical
                if self.scroll_accumulator_y.abs() >= threshold {
                    let scroll_y = (self.scroll_accumulator_y / threshold).trunc() as i32;
                    self.scroll_accumulator_y %= threshold;

                    // Si notas que el scroll va al revés (por el Natural Scrolling de Mac),
                    // cambia 'scroll_y' por '-scroll_y'
                    let _ = self.enigo.scroll(-scroll_y, Axis::Vertical);
                }

                // Evaluamos y ejecutamos el Scroll Horizontal
                if self.scroll_accumulator_x.abs() >= threshold {
                    let scroll_x = (self.scroll_accumulator_x / threshold).trunc() as i32;
                    self.scroll_accumulator_x %= threshold;

                    let _ = self.enigo.scroll(scroll_x, Axis::Horizontal);
                }
            }

            // Mapeamos los Swipes a las teclas Control + Flechas (Mission Control / Escritorios)
            ActionType::SwipeUp => self.send_shortcut(Key::UpArrow),
            ActionType::SwipeDown => self.send_shortcut(Key::DownArrow),

            // Invertimos Izquierda/Derecha intencionalmente para que coincida con el gesto natural de la mano
            ActionType::SwipeLeft => self.send_shortcut(Key::RightArrow),
            ActionType::SwipeRight => self.send_shortcut(Key::LeftArrow),

            ActionType::NoAction => {
                self.scroll_accumulator_y = 0.0;
                self.scroll_accumulator_x = 0.0;
            }

            // Catch-all por si el Enum ActionType crece en el futuro
            _ => {}
        }
    }
}
