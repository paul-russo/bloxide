mod bag_manager;
mod block;
mod draw;
mod game_state;
mod grid;
mod piece;
mod utils;

use draw::Drawable;
use game_state::{GameInput, GameState};
use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: String::from("Retris"),
        high_dpi: true,
        window_resizable: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // Game state
    let mut game_state = GameState::new();

    loop {
        clear_background(BLACK);

        game_state.update(GameInput {
            soft_drop: is_key_down(KeyCode::Down),
            shift_left: is_key_down(KeyCode::Left),
            shift_right: is_key_down(KeyCode::Right),
            rotate_right: is_key_pressed(KeyCode::Up),
            hard_drop: is_key_pressed(KeyCode::Space),
            hold_piece: is_key_pressed(KeyCode::C),
            toggle_pause: is_key_pressed(KeyCode::Escape),
        });

        game_state.draw(());
        game_state.clean_up();

        next_frame().await
    }
}
