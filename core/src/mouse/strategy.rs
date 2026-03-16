pub trait MouseStrategy: Send {
    fn move_cursor(&mut self, delta_x: f32, delta_y: f32);
    fn execute_click(&mut self, action_id: i32, phase_id: i32);
}
