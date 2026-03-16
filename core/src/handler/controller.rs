use crate::mouse::strategy::MouseStrategy;

pub struct InputController {
    // Guardamos la estrategia dinámicamente
    strategy: Box<dyn MouseStrategy>,
}

impl InputController {
    pub fn new(strategy: Box<dyn MouseStrategy>) -> Self {
        Self { strategy }
    }

    pub fn move_mouse(&mut self, delta_x: f32, delta_y: f32) {
        self.strategy.move_cursor(delta_x, delta_y);
    }

    pub fn execute_action(&mut self, action: i32, phase: i32) {
        self.strategy.execute_click(action, phase);
    }
}
