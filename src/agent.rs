//! Text-driven play for agents and scripts.
//!
//! `--agent` starts a run immediately and reads whitespace-separated commands
//! from stdin, one line per batch. Gravity and the lock delay are frozen for
//! the run, so the active piece waits wherever it is until it is hard-dropped:
//! a caller that needs seconds to decide plays the same game as one that
//! needs milliseconds. After every line the well is printed to stdout as
//! text, along with the active piece, hold slot, preview queue and score, so
//! nothing has to be read back off the screen. The window stays up and the
//! keyboard keeps working, so a person can watch or step in.
//!
//! Commands are case-insensitive. `l`/`r` shift, `cw`/`ccw` rotate (`x`/`z`
//! as on the keyboard), `drop` hard-drops, `hold` swaps the hold slot, `new`
//! starts a fresh run, `pause` toggles the pause menu, `state` reprints the
//! well without moving, and `quit` exits. Shifts and rotations take a repeat
//! count suffix (`l3`, `cw2`). A `#` starts a comment.
//!
//! `--agent=PATH` reads the same commands from a file instead, following it
//! like `tail -f`: each appended line is one turn. That avoids the two ways a
//! pipe goes wrong for a caller issuing turns from separate shell commands:
//! the game seeing EOF when the writer closes, and a relay such as `tail -f`
//! block-buffering its output so lines never arrive.

use crate::game_state::{GameInput, GameState};
use crate::grid::{FIRST_VISIBLE_ROW_ID, GRID_COUNT_COLS, GRID_COUNT_ROWS};
use crate::piece::{pieces, Piece};
use macroquad::prelude::Color;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

/// Where agent commands come from, as selected on the command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandInput {
    Stdin,
    File(PathBuf),
}

/// The agent command source requested by `--agent` / `--agent=PATH`, if any.
pub fn command_input_from_args() -> Option<CommandInput> {
    std::env::args().find_map(|arg| {
        if arg == "--agent" {
            return Some(CommandInput::Stdin);
        }

        arg.strip_prefix("--agent=")
            .filter(|path| !path.is_empty())
            .map(|path| CommandInput::File(PathBuf::from(path)))
    })
}

/// One parsed stdin command.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Command {
    ShiftLeft(usize),
    ShiftRight(usize),
    RotateClockwise(usize),
    RotateCounterclockwise(usize),
    HardDrop,
    Hold,
    NewGame,
    TogglePause,
    ShowState,
    Quit,
}

impl Command {
    /// The input sample that performs this command once, for commands that
    /// act on the active piece. Menu-level commands return `None`.
    pub fn piece_input(self) -> Option<GameInput> {
        let input = match self {
            Command::ShiftLeft(_) => GameInput {
                shift_left: true,
                ..Default::default()
            },
            Command::ShiftRight(_) => GameInput {
                shift_right: true,
                ..Default::default()
            },
            Command::RotateClockwise(_) => GameInput {
                rotate_right: true,
                ..Default::default()
            },
            Command::RotateCounterclockwise(_) => GameInput {
                rotate_left: true,
                ..Default::default()
            },
            Command::HardDrop => GameInput {
                hard_drop: true,
                ..Default::default()
            },
            Command::Hold => GameInput {
                hold_piece: true,
                ..Default::default()
            },
            Command::NewGame | Command::TogglePause | Command::ShowState | Command::Quit => {
                return None
            }
        };

        Some(input)
    }

    /// How many times `piece_input` should be applied.
    pub fn repeat_count(self) -> usize {
        match self {
            Command::ShiftLeft(count)
            | Command::ShiftRight(count)
            | Command::RotateClockwise(count)
            | Command::RotateCounterclockwise(count) => count,
            _ => 1,
        }
    }
}

/// Split a token such as `l3` into its word and optional repeat count.
fn split_repeat(token: &str) -> Result<(&str, usize), String> {
    let digits_start = token
        .char_indices()
        .find(|(_, c)| c.is_ascii_digit())
        .map_or(token.len(), |(index, _)| index);
    let (word, digits) = token.split_at(digits_start);
    if digits.is_empty() {
        return Ok((word, 1));
    }

    let count: usize = digits
        .parse()
        .map_err(|_| format!("bad repeat count in `{token}`"))?;
    if count == 0 {
        return Err(format!("repeat count must be at least 1 in `{token}`"));
    }

    Ok((word, count))
}

/// Parse one line of commands. The whole line is rejected on any unknown
/// token so a typo never leaves a batch half-applied.
pub fn parse_line(line: &str) -> Result<Vec<Command>, String> {
    let content = line.split('#').next().unwrap_or("");
    let mut commands = Vec::new();

    for token in content.split_whitespace() {
        let lowered = token.to_ascii_lowercase();
        let (word, count) = split_repeat(&lowered)?;
        let command = match word {
            "l" | "left" => Command::ShiftLeft(count),
            "r" | "right" => Command::ShiftRight(count),
            "cw" | "x" => Command::RotateClockwise(count),
            "ccw" | "z" => Command::RotateCounterclockwise(count),
            "drop" | "d" | "space" => Command::HardDrop,
            "hold" | "c" => Command::Hold,
            "new" => Command::NewGame,
            "pause" | "esc" => Command::TogglePause,
            "state" | "s" | "?" => Command::ShowState,
            "quit" | "q" => Command::Quit,
            _ => return Err(format!("unknown command `{token}`")),
        };
        if command.repeat_count() != count {
            return Err(format!("`{token}` does not take a repeat count"));
        }

        commands.push(command);
    }

    Ok(commands)
}

/// How long the file follower sleeps at end of file before looking for more.
const FILE_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Lines waiting to be run, collected on a background thread so the frame
/// loop never blocks on a read.
pub struct CommandSource {
    receiver: Receiver<String>,
    closed: bool,
}

impl CommandSource {
    /// Start reading commands. Stdin ends at EOF; a file is followed
    /// indefinitely. Either way the frame loop keeps running on keyboard
    /// input once the source is gone.
    pub fn spawn(input: CommandInput) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel();

        match input {
            CommandInput::Stdin => {
                thread::spawn(move || {
                    let stdin = std::io::stdin();
                    for line in stdin.lock().lines() {
                        let Ok(line) = line else { break };
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                });
            }
            CommandInput::File(path) => {
                let file = File::open(&path)?;
                thread::spawn(move || follow_file(file, &path, sender));
            }
        }

        Ok(Self {
            receiver,
            closed: false,
        })
    }

    /// Every line that arrived since the last poll, oldest first.
    pub fn pending_lines(&mut self) -> Vec<String> {
        let mut lines = Vec::new();

        loop {
            match self.receiver.try_recv() {
                Ok(line) => lines.push(line),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.closed = true;
                    break;
                }
            }
        }

        lines
    }

    /// True once the source is gone and no more lines can arrive.
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

/// Deliver each complete line appended to `file`, from its start, forever.
/// A line is only sent once its newline has been written, so a writer that
/// is mid-`echo` when the poll lands is not seen as two half commands.
fn follow_file(file: File, path: &Path, sender: Sender<String>) {
    let mut reader = BufReader::new(file);
    let mut pending = String::new();
    let mut chunk = String::new();

    loop {
        chunk.clear();
        match reader.read_line(&mut chunk) {
            Ok(0) => thread::sleep(FILE_POLL_INTERVAL),
            Ok(_) => {
                pending.push_str(&chunk);
                if !pending.ends_with('\n') {
                    continue;
                }

                let line = pending.trim_end_matches(['\n', '\r']).to_string();
                pending.clear();
                if sender.send(line).is_err() {
                    break;
                }
            }
            Err(error) => {
                eprintln!("AGENT stopped following {}: {error}", path.display());
                break;
            }
        }
    }
}

/// Cell glyph for a locked block, recovered from its colour. Blocks carry
/// only a colour, so this is the one place the palette is read back.
fn piece_letter(color: Color) -> char {
    const PALETTE: [(Color, char); 7] = [
        (pieces::PIECE_COLOR_I, 'I'),
        (pieces::PIECE_COLOR_J, 'J'),
        (pieces::PIECE_COLOR_L, 'L'),
        (pieces::PIECE_COLOR_O, 'O'),
        (pieces::PIECE_COLOR_S, 'S'),
        (pieces::PIECE_COLOR_T, 'T'),
        (pieces::PIECE_COLOR_Z, 'Z'),
    ];

    PALETTE
        .iter()
        .find(|(candidate, _)| *candidate == color)
        .map_or('#', |(_, letter)| *letter)
}

fn piece_name(piece: Option<Piece>) -> &'static str {
    piece.map_or("-", |piece| piece.name)
}

/// The well and run status as text. One header line of `key=value` pairs,
/// then the visible rows top to bottom with the active piece drawn as `@`,
/// its landing ghost as `:`, locked blocks as their piece letter and empty
/// cells as `.`. Rows are numbered from the top of the visible well, so
/// printed row `n` is grid row `FIRST_VISIBLE_ROW_ID + n`. A piece spawns
/// with its upper cells in the hidden buffer just above the well; any hidden
/// row the active piece occupies is printed too, with a negative number, so
/// the whole piece is always in view.
pub fn render_state(game_state: &GameState) -> String {
    let (row, col, orientation) = game_state.get_active_piece_pose();
    let previews = game_state.get_piece_previews();
    let mut out = format!(
        "STATE piece={} orient={} row={} col={} hold={} next={},{},{} score={} lines={} level={} paused={} over={}\n",
        game_state.get_active_piece().name,
        orientation,
        row,
        col,
        piece_name(game_state.get_held_piece()),
        previews[0].name,
        previews[1].name,
        previews[2].name,
        game_state.get_score(),
        game_state.get_rows_cleared(),
        game_state.get_level(),
        yes_no(game_state.get_is_paused()),
        yes_no(game_state.get_is_game_over()),
    );

    out.push_str("  |");
    for col in 0..GRID_COUNT_COLS {
        out.push(char::from_digit(col as u32, 10).unwrap_or('?'));
    }
    out.push_str("|\n");

    let locked = game_state.get_grid_locked();
    let active = game_state.get_grid_active();
    let ghost = game_state.get_grid_ghost();
    let top_row = (0..FIRST_VISIBLE_ROW_ID)
        .find(|&row| (0..GRID_COUNT_COLS).any(|col| active.has_block_at_cell(row, col)))
        .unwrap_or(FIRST_VISIBLE_ROW_ID);

    for grid_row in top_row..GRID_COUNT_ROWS {
        let label = grid_row as isize - FIRST_VISIBLE_ROW_ID as isize;
        out.push_str(&format!("{label:02}|"));

        for col in 0..GRID_COUNT_COLS {
            let glyph = if let Some(block) = locked.get_cell(grid_row, col) {
                piece_letter(block.color)
            } else if active.has_block_at_cell(grid_row, col) {
                '@'
            } else if ghost.has_block_at_cell(grid_row, col) {
                ':'
            } else {
                '.'
            };
            out.push(glyph);
        }

        out.push_str("|\n");
    }

    out.push_str("  +");
    out.push_str(&"-".repeat(GRID_COUNT_COLS));
    out.push_str("+\n");

    out
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

/// Command reference printed once at startup so a caller that only has the
/// pipe still knows the vocabulary.
pub const USAGE: &str = "\
AGENT bloxide agent mode: gravity is frozen; one line of commands per turn.
AGENT commands: l r cw ccw drop hold new pause state quit  (repeat: l3, cw2; # comment)";

#[cfg(test)]
mod tests {
    use super::{parse_line, render_state, Command, CommandInput, CommandSource};
    use crate::game_state::{GameInput, GameState};
    use crate::grid::{GRID_COUNT_COLS, VISIBLE_GRID_COUNT_ROWS};
    use crate::high_score_manager::HighScoreManager;
    use std::io::Write;
    use std::time::{Duration, Instant};

    #[test]
    fn a_followed_file_delivers_whole_appended_lines_only() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("bloxide-agent-{}.cmds", std::process::id()));
        std::fs::write(&path, "l2 drop\n").unwrap();
        let mut source = CommandSource::spawn(CommandInput::File(path.clone())).unwrap();

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(b"cw").unwrap();
        file.flush().unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut lines = Vec::new();
        while lines.len() < 1 && Instant::now() < deadline {
            lines.extend(source.pending_lines());
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(lines, vec!["l2 drop".to_string()]);

        file.write_all(b" drop\n").unwrap();
        file.flush().unwrap();
        while lines.len() < 2 && Instant::now() < deadline {
            lines.extend(source.pending_lines());
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(lines[1], "cw drop");
        assert!(!source.is_closed());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn parses_a_batch_with_repeat_counts_and_aliases() {
        let commands = parse_line("L3 x drop  # slide over and slam").unwrap();

        assert_eq!(
            commands,
            vec![
                Command::ShiftLeft(3),
                Command::RotateClockwise(1),
                Command::HardDrop
            ]
        );
    }

    #[test]
    fn rejects_the_whole_line_on_an_unknown_token() {
        assert!(parse_line("l2 wat drop").is_err());
        assert!(parse_line("drop3").is_err());
        assert!(parse_line("l0").is_err());
        assert_eq!(parse_line("   # nothing").unwrap(), vec![]);
    }

    #[test]
    fn menu_commands_have_no_piece_input() {
        assert!(Command::NewGame.piece_input().is_none());
        assert!(Command::Quit.piece_input().is_none());
        assert!(Command::HardDrop.piece_input().unwrap().hard_drop);
        assert_eq!(Command::RotateCounterclockwise(2).repeat_count(), 2);
    }

    #[test]
    fn renders_header_and_every_visible_row() {
        let high_scores = HighScoreManager::new();
        let mut state = GameState::new(&high_scores);
        state.freeze_gravity();
        state.update(GameInput::default());

        let text = render_state(&state);
        let lines: Vec<&str> = text.lines().collect();

        assert!(lines[0].starts_with("STATE piece="));
        assert!(lines[0].contains("over=no"));
        assert_eq!(lines[1], "  |0123456789|");
        assert!(lines.len() >= 3 + VISIBLE_GRID_COUNT_ROWS);
        assert_eq!(lines[2].len(), GRID_COUNT_COLS + 4);
        assert_eq!(lines.last().unwrap(), &"  +----------+");
        assert!(text.contains('@'), "active piece should be drawn");
        assert!(text.contains(':'), "landing ghost should be drawn");

        // Every cell of the freshly spawned piece is in view, including the
        // ones still in the hidden buffer, and the visible rows follow.
        let active_cells = text.matches('@').count();
        assert_eq!(active_cells, 4);
        assert!(lines.iter().any(|line| line.starts_with("00|")));
        assert!(lines.iter().any(|line| line.starts_with("19|")));
    }

    #[test]
    fn repeated_shifts_move_one_column_each_when_released_between_presses() {
        let high_scores = HighScoreManager::new();
        let mut state = GameState::new(&high_scores);
        state.freeze_gravity();
        let (_, col_before, _) = state.get_active_piece_pose();

        let input = Command::ShiftLeft(3).piece_input().unwrap();
        for _ in 0..3 {
            state.update(input);
            state.update(GameInput::default());
        }

        let (_, col_after, _) = state.get_active_piece_pose();
        assert_eq!(col_after, col_before - 3);
    }

    #[test]
    fn frozen_gravity_holds_the_piece_until_a_hard_drop() {
        let high_scores = HighScoreManager::new();
        let mut state = GameState::new(&high_scores);
        state.freeze_gravity();

        let (row_before, ..) = state.get_active_piece_pose();
        state.update_with_elapsed(std::time::Duration::from_secs(30), GameInput::default());
        let (row_after, ..) = state.get_active_piece_pose();
        assert_eq!(row_before, row_after);
        assert!(!state.get_is_game_over());

        state.update(Command::HardDrop.piece_input().unwrap());
        let locked = state.get_grid_locked();
        let bottom_filled = (0..GRID_COUNT_COLS)
            .filter(|&col| locked.has_block_at_cell(crate::grid::GRID_COUNT_ROWS - 1, col))
            .count();
        assert!(bottom_filled >= 2);
    }
}
