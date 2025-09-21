use macroquad::prelude::*;

#[macroquad::main("Physarum")]
async fn main() {
    loop {
        clear_background(DARKPURPLE);
        next_frame().await;
    }
}
