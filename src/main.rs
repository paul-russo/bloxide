mod bag_manager;
mod block;
mod draw;
mod game_state;
mod grid;
mod menu;
mod piece;

use std::cell::Cell;

use draw::Drawable;
use game_state::{GameInput, GameState};
use macroquad::{miniquad::window::quit, prelude::*};
use menu::{Menu, MenuInput, MenuItem};

fn window_conf() -> Conf {
    Conf {
        window_title: String::from("Retris"),
        high_dpi: true,
        window_resizable: false,
        ..Default::default()
    }
}

#[derive(PartialEq)]
enum CurrentScreen {
    Game,
    MainMenu,
}

#[macroquad::main(window_conf)]
async fn main() {
    // Putting this value inside of a Cell allows us to mutate the inner value without
    // needing to use a mutable reference, which would cause on_end to be an FnMut instead
    // of an Fn.
    let high_score: Cell<usize> = Cell::new(0);

    let on_end = |score: usize| {
        if score > high_score.get() {
            high_score.set(score);
        }

        println!("HIGH SCORE: {}", high_score.get());
    };

    let mut current_screen = CurrentScreen::MainMenu;

    // Game state
    let mut maybe_game_state: Option<GameState> = None;

    let mut menu_main = Menu::new(
        "RETRIS",
        vec![
            MenuItem {
                label: "New Game",
                id: "new_game",
            },
            MenuItem {
                label: "Quit",
                id: "quit",
            },
        ],
    );

    menu_main.is_visible = true;

    let mut menu_game_over = Menu::new(
        "GAME OVER",
        vec![
            MenuItem {
                label: "New Game",
                id: "new_game",
            },
            MenuItem {
                label: "Main Menu",
                id: "back_to_main_menu",
            },
            MenuItem {
                label: "Quit",
                id: "quit",
            },
        ],
    );

    let mut menu_paused = Menu::new(
        "PAUSED",
        vec![
            MenuItem {
                label: "Resume",
                id: "resume",
            },
            MenuItem {
                label: "Main Menu",
                id: "back_to_main_menu",
            },
            MenuItem {
                label: "Quit",
                id: "quit",
            },
        ],
    );

    loop {
        clear_background(BLACK);

        let menu_input = MenuInput {
            up: is_key_pressed(KeyCode::Up),
            down: is_key_pressed(KeyCode::Down),
            select: is_key_pressed(KeyCode::Enter),
        };

        if current_screen == CurrentScreen::Game && maybe_game_state.is_some() {
            let game_state = maybe_game_state.as_mut().unwrap();
            game_state.update(GameInput {
                soft_drop: is_key_down(KeyCode::Down),
                shift_left: is_key_down(KeyCode::Left),
                shift_right: is_key_down(KeyCode::Right),
                rotate_right: is_key_pressed(KeyCode::Up),
                hard_drop: is_key_pressed(KeyCode::Space),
                hold_piece: is_key_pressed(KeyCode::C),
                toggle_pause: is_key_pressed(KeyCode::Escape),
            });

            menu_game_over.is_visible = game_state.get_is_game_over();
            menu_paused.is_visible = game_state.get_is_paused();

            match menu_game_over.update(menu_input) {
                Some("new_game") => *game_state = GameState::new(&on_end),
                Some("back_to_main_menu") => current_screen = CurrentScreen::MainMenu,
                Some("quit") => quit(),
                _ => (),
            }

            match menu_paused.update(menu_input) {
                Some("resume") => game_state.toggle_pause(),
                Some("back_to_main_menu") => current_screen = CurrentScreen::MainMenu,
                Some("quit") => quit(),
                _ => (),
            }

            game_state.draw(());
            menu_game_over.draw(());
            menu_paused.draw(());

            game_state.clean_up();
        } else {
            match menu_main.update(menu_input) {
                Some("new_game") => {
                    current_screen = CurrentScreen::Game;
                    maybe_game_state = Some(GameState::new(&on_end));
                }
                Some("quit") => quit(),
                _ => (),
            }

            menu_main.draw(());
        }

        next_frame().await
    }
}
