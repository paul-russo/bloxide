mod bag_manager;
mod block;
mod draw;
mod effects;
mod game_state;
mod grid;
mod high_score_manager;
mod lighting;
mod menu;
mod piece;
mod pixel_font;
mod postfx;
mod render3d;
mod textures;

use draw::{draw_backdrop, Drawable, RenderSurface, WINDOW_HEIGHT, WINDOW_WIDTH};
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

/// Which frame `--screenshot` captures before quitting. Each scene exercises a
/// different slice of the renderer so visual changes can be checked headlessly.
#[derive(Copy, Clone, PartialEq)]
enum ScreenshotScene {
    /// A seeded stack mid line-clear, with shrapnel and shake in flight.
    Carnage,
    /// The same seeded stack at rest, with nothing clearing.
    Still,
    /// A topped-out stack with the game over menu up.
    GameOver,
    /// The main menu over the empty well.
    Menu,
}

fn screenshot_scene_from_args() -> Option<ScreenshotScene> {
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|arg| arg == "--screenshot") {
        return None;
    }

    if args.iter().any(|arg| arg == "--menu") {
        Some(ScreenshotScene::Menu)
    } else if args.iter().any(|arg| arg == "--still") {
        Some(ScreenshotScene::Still)
    } else if args.iter().any(|arg| arg == "--gameover") {
        Some(ScreenshotScene::GameOver)
    } else {
        Some(ScreenshotScene::Carnage)
    }
}

/// How many frames to run before capturing, from `--frame=N`. Effects such as
/// the clear flash and shrapnel burst evolve over the first second, so this
/// picks which moment of them to inspect.
fn screenshot_frame_from_args() -> usize {
    const DEFAULT_FRAME: usize = 16;
    std::env::args()
        .find_map(|arg| arg.strip_prefix("--frame=").and_then(|n| n.parse().ok()))
        .unwrap_or(DEFAULT_FRAME)
}

/// Seed a partially filled stack for screenshots. `Carnage` leaves the bottom
/// rows complete so a line clear can be triggered; `Still` punches a column out
/// so nothing is clearable and the stack simply sits there; `GameOver` piles
/// the stack to the ceiling and ends the run.
fn seed_screenshot_state<'a>(
    scene: ScreenshotScene,
    high_score_manager: &'a HighScoreManager,
) -> GameState<'a> {
    let mut game_state = GameState::new(high_score_manager);
    let colors = [
        piece::pieces::PIECE_COLOR_I,
        piece::pieces::PIECE_COLOR_J,
        piece::pieces::PIECE_COLOR_L,
        piece::pieces::PIECE_COLOR_O,
        piece::pieces::PIECE_COLOR_S,
        piece::pieces::PIECE_COLOR_T,
        piece::pieces::PIECE_COLOR_Z,
    ];
    let first_row = if scene == ScreenshotScene::GameOver { 4 } else { 17 };

    for row in first_row..22 {
        for col in 0..10 {
            let gap = (row == 17 && (col == 2 || col == 3 || col == 7))
                || (row == 18 && (col == 4 || col == 5))
                || (scene == ScreenshotScene::Still && col == 4)
                || (scene == ScreenshotScene::GameOver && (row * 7 + col * 3) % 5 == 0);
            if gap {
                continue;
            }

            game_state.get_grid_locked_mut().set_cell(
                row,
                col,
                Some(block::Block::new(colors[(row * 3 + col) % colors.len()])),
            );
        }
    }

    match scene {
        ScreenshotScene::Carnage => game_state.trigger_line_clear(),
        ScreenshotScene::GameOver => game_state.trigger_game_over(),
        ScreenshotScene::Still | ScreenshotScene::Menu => {}
    }

    game_state
}

/// Alpha-blended overlays leave partial alpha in the framebuffer, which image
/// viewers then composite against their own backdrop. Force the export opaque
/// so the PNG shows exactly what was on screen.
fn export_opaque_png(mut image: Image, path: &str) {
    for pixel in image.get_image_data_mut() {
        pixel[3] = 255;
    }
    image.export_png(path);
}

#[macroquad::main(window_conf)]
async fn main() {
    let high_score_manager = HighScoreManager::new();
    let render_surface = RenderSurface::new();
    let mut current_screen = CurrentScreen::MainMenu;

    // Game state
    let mut maybe_game_state: Option<GameState> = None;
    let screenshot_scene = screenshot_scene_from_args();
    let screenshot_capture_frame = screenshot_frame_from_args();
    let mut screenshot_frame: usize = 0;

    match screenshot_scene {
        Some(ScreenshotScene::Menu) | None => {}
        Some(scene) => {
            current_screen = CurrentScreen::Game;
            maybe_game_state = Some(seed_screenshot_state(scene, &high_score_manager));
        }
    }

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
            menu_game_over.draw(render_surface.clone());
            menu_paused.draw(render_surface.clone());

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

            menu_main.draw(render_surface.clone());
            high_score_manager.draw(render_surface.clone());
        }

        render_surface.present();

        if is_key_pressed(KeyCode::F12) {
            export_opaque_png(get_screen_data(), "screenshot.png");
        }

        if screenshot_scene.is_some() {
            screenshot_frame += 1;
            if screenshot_frame >= screenshot_capture_frame {
                export_opaque_png(get_screen_data(), "screenshot.png");
                export_opaque_png(
                    render_surface.target.texture.get_texture_data(),
                    "screenshot-render-target.png",
                );
                quit();
            }
        }

        next_frame().await
    }
}
