mod bag_manager;
mod block;
mod draw;
mod game_state;
mod grid;
mod high_score_manager;
mod menu;
mod piece;
mod render3d;

use draw::{draw_backdrop, draw_background, Drawable, RenderSurface, WINDOW_HEIGHT, WINDOW_WIDTH};
use game_state::{GameInput, GameState};
use high_score_manager::HighScoreManager;
use macroquad::{miniquad::window::quit, prelude::*};
use menu::{Menu, MenuInput, MenuItem};

fn window_conf() -> Conf {
    Conf {
        window_title: String::from("BLOXIDE // Software Carnage"),
        high_dpi: true,
        window_resizable: false,
        window_height: WINDOW_HEIGHT as i32,
        window_width: WINDOW_WIDTH as i32,
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
    let high_score_manager = HighScoreManager::new();
    let render_surface = RenderSurface::new();
    let mut current_screen = CurrentScreen::MainMenu;

    // Game state
    let mut maybe_game_state: Option<GameState> = None;

    let mut menu_main = Menu::new(
        "bloxide",
        vec![
            MenuItem {
                label: "Start Run",
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
        render_surface.begin_frame();
        draw_background();

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
                Some("new_game") => *game_state = GameState::new(&high_score_manager),
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

            game_state.draw(render_surface.clone());
            menu_game_over.draw(());
            menu_paused.draw(());

            game_state.clean_up();
        } else {
            match menu_main.update(menu_input) {
                Some("new_game") => {
                    current_screen = CurrentScreen::Game;
                    maybe_game_state = Some(GameState::new(&high_score_manager));
                }
                Some("quit") => quit(),
                _ => (),
            }

            // The empty well is drawn behind the main menu so the 3D playfield
            // frames the menu screen too, rather than it floating on a void.
            draw_backdrop(&render_surface);

            menu_main.draw(());
            high_score_manager.draw(());
        }

        render_surface.present();
        next_frame().await
    }
}
