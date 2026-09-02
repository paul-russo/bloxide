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

## Headless validation harness

The binary has a built-in harness for validating rendering and performance
without a human at the keyboard — the best way to check the environment works:

- Screenshot a seeded scene and quit: `DISPLAY=:1 ./target/debug/bloxide --screenshot`.
  Writes `screenshot.png` (and `screenshot-render-target.png`) to the working
  directory. Add `--frame=N` to pick the captured frame.
- Scene flags: `--still`, `--gameover`, `--menu` (default is the mid-line-clear
  "carnage" scene).
- Performance telemetry over N frames: `DISPLAY=:1 ./target/debug/bloxide --telemetry --frames=N`.

## Non-obvious caveats

- **Display required**: Macroquad needs an X display. The cloud VM provides one
  on `:1` (`DISPLAY=:1`), so prefix run/screenshot/telemetry commands with
  `DISPLAY=:1`. Building and testing do **not** need a display.
- **Toolchain**: build on `stable`, not the base image's pinned default (see
  above).
- **Clippy warnings**: the codebase currently emits ~15 clippy warnings
  (needless range loops, a `clamp`-like pattern, an `unwrap` after `is_some`).
  These are pre-existing, not regressions.
- **Tests**: `cargo test` runs the unit suite (currently 51 tests) and needs no
  display.
- **High scores**: the game writes a `.highscore` file in the working
  directory. It is gitignored.
