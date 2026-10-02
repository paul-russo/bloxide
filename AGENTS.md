# Agents

## Overview

**bloxide** is a single-binary Tetris-style block puzzle game written in Rust
using [Macroquad](https://macroquad.rs/) (OpenGL via miniquad). There are no
backend services, databases, or network dependencies — everything runs in one
`cargo` binary.

## Cloud environment

The Cloud Agent environment is configured by [`.cursor/environment.json`](.cursor/environment.json),
which runs [`.cursor/install.sh`](.cursor/install.sh). That script:

- Installs the X11/OpenGL/ALSA development headers Macroquad needs to build
  (`libgl1-mesa-dev`, `libxi-dev`, `libasound2-dev`, `libxcursor-dev`,
  `libxrandr-dev`, `libxinerama-dev`).
- Selects the current Rust **stable** toolchain. The base image's default
  toolchain is too old for the dependency tree (`fontdue` uses
  `integer_sign_cast`, stabilized in Rust 1.87), so building on stale stable
  fails with `error[E0658]`.
- Runs `cargo build`.

## Standard commands

| Action | Command |
|--------|---------|
| Build | `cargo build` |
| Build (release) | `cargo build --release` |
| Test | `cargo test` |
| Lint | `cargo clippy` |
| Run (interactive) | `DISPLAY=:1 cargo run` |
| Show window | `DISPLAY=:1 cargo run -- --visible` |
| Play from a pipe | `DISPLAY=:1 cargo run -- --agent` (see below) |

## Headless validation harness

The binary has a built-in harness for validating rendering and performance
without a human at the keyboard — the best way to check the environment works:

- Screenshot a seeded scene and quit: `DISPLAY=:1 ./target/debug/bloxide --screenshot`.
  Writes `screenshot.png` (and `screenshot-render-target.png`) to the working
  directory. Add `--frame=N` to pick the captured frame.
- Add `--sequence --frame=N` to a screenshot run to export every frame as
  `screenshot-0001.png`, etc. Screenshot simulation advances at a fixed 60 Hz
  so the heat-up, fracture, and lava landing can be reviewed in motion.
- Scene flags: `--still`, `--gameover`, `--menu` (default is the mid-line-clear
  "carnage" scene).
- Performance telemetry over N frames: `DISPLAY=:1 ./target/debug/bloxide --telemetry --frames=N`.

## Agent mode (text-driven play)

`--agent` is how an agent or script plays a real game without reading pixels
or racing the clock. It skips the menu, starts a run with **gravity and the
lock delay frozen** (the active piece waits until it is hard-dropped), reads
whitespace-separated commands from stdin one line per turn, and prints the
board as text after every line. The window still opens and the keyboard still
works, so a person can watch or take over.

- Commands (case-insensitive): `l` `r` shift, `cw` `ccw` rotate (`x`/`z`),
  `drop` hard-drop, `hold`, `new`, `pause`, `state` (reprint), `quit`.
  Shifts and rotations take a repeat count: `l3 cw2 drop`. `#` starts a
  comment. A line with an unknown token is rejected whole (`ERR ...`).
- Output per line: a `STATE key=value ...` header (active piece, orientation,
  canvas origin `row`/`col` in full-grid coordinates, hold, next three, score,
  lines, level, paused, over), then the 20 visible rows numbered from the top
  with `@` active piece, `:` landing ghost, piece letters for locked blocks and
  `.` for empty. `GAME OVER score=N` follows a topped-out board.
- `--agent=PATH` follows a command file instead of stdin (like `tail -f`),
  which is the reliable choice when turns come from separate shell commands:
  `: > /tmp/bloxide.cmds; ./target/release/bloxide --agent=/tmp/bloxide.cmds
  > /tmp/bloxide.out &` then `echo "l3 drop" >> /tmp/bloxide.cmds` per turn.
  Plain `--agent` reads stdin and stops at EOF (the game keeps running on the
  keyboard and prints `AGENT command source closed`); a pipe relay such as
  `tail -f | bloxide --agent` block-buffers on macOS, so prefer the file.

## Non-obvious caveats

- **Display required**: Macroquad needs an X display. The cloud VM provides one
  on `:1` (`DISPLAY=:1`), so prefix run/screenshot/telemetry commands with
  `DISPLAY=:1`. Building and testing do **not** need a display.
- **Toolchain**: build on `stable`, not the base image's pinned default (see
  above).
- **Clippy warnings**: the codebase currently emits ~15 clippy warnings
  (needless range loops, a `clamp`-like pattern, an `unwrap` after `is_some`).
  These are pre-existing, not regressions.
- **Tests**: `cargo test` runs the unit suite (currently 169 tests) and needs no
  display.
- **High scores**: the game writes a `.highscore` file in the working
  directory. It is gitignored.
