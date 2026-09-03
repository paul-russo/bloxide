mod bag_manager;
mod block;
mod draw;
mod effects;
mod game_state;
mod grid;
mod headless;
mod high_score_manager;
mod lighting;
mod menu;
mod piece;
mod pixel_font;
mod postfx;
mod render3d;
mod telemetry;
mod textures;

use draw::{camera_shake, draw_backdrop, Drawable, RenderSurface, WINDOW_HEIGHT, WINDOW_WIDTH};
use game_state::{GameInput, GameState};
use grid::{FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS, VISIBLE_GRID_COUNT_ROWS};
use high_score_manager::HighScoreManager;
use macroquad::{miniquad::window::quit, prelude::*};
use menu::{Menu, MenuInput, MenuItem};
use render3d::{FRAME_INDEX_CAPACITY, FRAME_VERTEX_CAPACITY};
use telemetry::{Phase, Telemetry};

fn window_conf() -> macroquad::conf::Conf {
    // Harness runs are launched repeatedly from a shell; keep their window
    // from appearing and stealing focus unless `--visible` asks for it. This
    // has to happen here, before the macroquad wrapper around `main` creates
    // the window.
    let wants_window = std::env::args().any(|arg| arg == "--visible");
    if harness_scene_from_args().is_some() && !wants_window {
        headless::install();
    }

    macroquad::conf::Conf {
        miniquad_conf: Conf {
            window_title: String::from("BLOXIDE // Software Carnage"),
            high_dpi: true,
            window_resizable: false,
            window_height: WINDOW_HEIGHT as i32,
            window_width: WINDOW_WIDTH as i32,
            ..Default::default()
        },
        // The renderer keeps a whole frame in a few batches; each batch has
        // to be able to hold it.
        draw_call_vertex_capacity: FRAME_VERTEX_CAPACITY,
        draw_call_index_capacity: FRAME_INDEX_CAPACITY,
        ..Default::default()
    }
}

#[derive(PartialEq)]
enum CurrentScreen {
    Game,
    MainMenu,
}

/// The seeded scene a harness run plays: the frame `--screenshot` captures
/// before quitting, or the frames `--telemetry` times. Each scene exercises a
/// different slice of the renderer so visual and performance changes can be
/// checked headlessly.
#[derive(Copy, Clone, PartialEq)]
enum HarnessScene {
    /// A seeded stack mid line-clear, with shrapnel and shake in flight.
    Carnage,
    /// The same seeded stack at rest, with nothing clearing.
    Still,
    /// A topped-out stack with the game over menu up.
    GameOver,
    /// The main menu over the empty well.
    Menu,
}

impl HarnessScene {
    fn label(self) -> &'static str {
        match self {
            HarnessScene::Carnage => "carnage",
            HarnessScene::Still => "still",
            HarnessScene::GameOver => "gameover",
            HarnessScene::Menu => "menu",
        }
    }
}

fn harness_scene_from_args() -> Option<HarnessScene> {
    if !is_screenshot_run() && telemetry::frames_from_args().is_none() {
        return None;
    }

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--menu") {
        Some(HarnessScene::Menu)
    } else if args.iter().any(|arg| arg == "--still") {
        Some(HarnessScene::Still)
    } else if args.iter().any(|arg| arg == "--gameover") {
        Some(HarnessScene::GameOver)
    } else {
        Some(HarnessScene::Carnage)
    }
}

fn is_screenshot_run() -> bool {
    std::env::args().any(|arg| arg == "--screenshot")
}

/// Clock step per frame in a screenshot run, retaining the 60 Hz capture cadence.
/// Time-driven effects are sampled at these fixed moments rather than the wall
/// clock, so a capture of frame N is reproducible.
const SCREENSHOT_FRAME_SECONDS: f64 = 1.0 / 60.0;

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
    scene: HarnessScene,
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
    let stack_top = VISIBLE_GRID_COUNT_ROWS - 5;
    let first_visible_row = if scene == HarnessScene::GameOver {
        2
    } else {
        stack_top
    };

    for visible_row in first_visible_row..VISIBLE_GRID_COUNT_ROWS {
        let row = FIRST_VISIBLE_ROW_ID + visible_row;
        // A fixed pattern phase preserves the fixture's colours and holes
        // independently of how many hidden rows the matrix contains.
        let pattern_row = visible_row + 2;

        for col in 0..GRID_COUNT_COLS {
            let gap = (visible_row == stack_top && (col == 2 || col == 3 || col == 7))
                || (visible_row == stack_top + 1 && (col == 4 || col == 5))
                || (scene == HarnessScene::Still && col == 4)
                || (scene == HarnessScene::GameOver && (pattern_row * 7 + col * 3) % 5 == 0);
            if gap {
                continue;
            }

            game_state.get_grid_locked_mut().set_cell(
                row,
                col,
                Some(block::Block::new(
                    colors[(pattern_row * 3 + col) % colors.len()],
                )),
            );
        }
    }

    match scene {
        HarnessScene::Carnage => game_state.trigger_line_clear(),
        HarnessScene::GameOver => game_state.trigger_game_over(),
        HarnessScene::Still | HarnessScene::Menu => {}
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
    let harness_scene = harness_scene_from_args();
    let is_screenshot = is_screenshot_run();
    let screenshot_capture_frame = screenshot_frame_from_args();
    let mut screenshot_frame: usize = 0;
    let mut telemetry = Telemetry::from_args(harness_scene.map_or("play", HarnessScene::label));

    match harness_scene {
        Some(HarnessScene::Menu) | None => {}
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
        if telemetry.begin_frame() {
            telemetry.report();
            quit();
        }

        // Screenshot runs advance the clock a fixed step per frame, so the
        // lightstyle flicker, embers and lava look the same on every run and
        // captures can be compared pixel for pixel.
        let time = if is_screenshot {
            screenshot_frame as f64 * SCREENSHOT_FRAME_SECONDS
        } else {
            get_time()
        };
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
        } else {
            match menu_main.update(menu_input) {
                Some("new_game") => {
                    current_screen = CurrentScreen::Game;
                    maybe_game_state = Some(GameState::new(&high_score_manager));
                }
                Some("quit") => quit(),
                _ => (),
            }
        }

        // The frame's camera shake comes from the updated game state, so the
        // frame can only begin once the update is done.
        telemetry.enter(Phase::Draw);
        let playing = match (&current_screen, maybe_game_state.as_mut()) {
            (CurrentScreen::Game, Some(game_state)) => Some(game_state),
            _ => None,
        };
        let shake = playing
            .as_deref()
            .map_or(Vec2::ZERO, |game_state| camera_shake(game_state, time));
        let frame = render_surface.begin_frame(time, shake);

        match playing {
            Some(game_state) => {
                game_state.draw(&frame);
                menu_game_over.draw(&frame);
                menu_paused.draw(&frame);
            }
            None => {
                // The empty well is drawn behind the main menu so the 3D
                // playfield frames the menu screen too, rather than it
                // floating on a void.
                draw_backdrop(&frame);
                menu_main.draw(&frame);
                high_score_manager.draw(&frame);
            }
        }

        telemetry.enter(Phase::Present);
        render_surface.present();
        telemetry.sync_gpu_then_wait();

        if is_key_pressed(KeyCode::F12) {
            export_opaque_png(get_screen_data(), "screenshot.png");
        }

        if is_screenshot {
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

#[cfg(test)]
mod tests {
    use super::{
        seed_screenshot_state, HarnessScene, HighScoreManager, FIRST_VISIBLE_ROW_ID,
        GRID_COUNT_COLS, VISIBLE_GRID_COUNT_ROWS,
    };
    use crate::grid::{Grid, GRID_COUNT_ROWS};

    fn occupied_in_row(grid: &Grid, row: usize) -> usize {
        (0..GRID_COUNT_COLS)
            .filter(|&col| grid.has_block_at_cell(row, col))
            .count()
    }

    #[test]
    fn seeded_stacks_remain_in_the_visible_well_when_the_buffer_grows() {
        let high_scores = HighScoreManager::new();

        for (scene, first_visible_row) in [
            (HarnessScene::Still, VISIBLE_GRID_COUNT_ROWS - 5),
            (HarnessScene::GameOver, 2),
        ] {
            let state = seed_screenshot_state(scene, &high_scores);
            let grid = state.get_grid_locked();
            let first_occupied = (0..GRID_COUNT_ROWS).find(|&row| occupied_in_row(grid, row) > 0);

            assert_eq!(
                first_occupied,
                Some(FIRST_VISIBLE_ROW_ID + first_visible_row)
            );
            assert!((0..FIRST_VISIBLE_ROW_ID).all(|row| occupied_in_row(grid, row) == 0));
            assert!(occupied_in_row(grid, GRID_COUNT_ROWS - 1) > 0);
            assert_eq!(state.get_rows_cleared(), 0);
            assert_eq!(state.get_is_game_over(), scene == HarnessScene::GameOver);
        }
    }

    #[test]
    fn seeded_line_clear_still_clears_three_rows_at_the_bottom() {
        let high_scores = HighScoreManager::new();
        let state = seed_screenshot_state(HarnessScene::Carnage, &high_scores);
        let grid = state.get_grid_locked();
        let occupied_rows: Vec<_> = (0..GRID_COUNT_ROWS)
            .filter(|&row| occupied_in_row(grid, row) > 0)
            .collect();
        let clear_mask = (VISIBLE_GRID_COUNT_ROWS - 3..VISIBLE_GRID_COUNT_ROWS)
            .fold(0, |mask, row| mask | (1 << row));

        assert_eq!(state.get_rows_cleared(), 3);
        assert_eq!(state.get_score(), 500);
        assert_eq!(state.get_clear_row_mask(), clear_mask);
        assert_eq!(
            occupied_rows,
            vec![GRID_COUNT_ROWS - 2, GRID_COUNT_ROWS - 1]
        );
        assert_eq!(occupied_in_row(grid, GRID_COUNT_ROWS - 2), 7);
        assert_eq!(occupied_in_row(grid, GRID_COUNT_ROWS - 1), 8);
    }
}
