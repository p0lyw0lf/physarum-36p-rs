use macroquad::miniquad;
use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Physarum".to_owned(),
        platform: miniquad::conf::Platform {
            // Gives about 48fps on my nvidia card
            swap_interval: Some(2),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        clear_background(BLACK);

        let frame_time = get_frame_time();
        let fps = 1. / frame_time;
        let text = format!("FPS: {fps}");
        draw_text(&text, 100., 100., 40., WHITE);
        println!("{}", text);

        if is_key_down(KeyCode::Escape) {
            break;
        }

        next_frame().await;
    }
}
