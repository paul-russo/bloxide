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
- Scene flags: `--still`, `--gameover`, `--paused`, `--menu` (default is the
  mid-line-clear "carnage" scene).
- Performance telemetry over N frames: `DISPLAY=:1 ./target/debug/bloxide --telemetry --frames=N`.
- `--portrait` renders the iPhone layout (302x656 framebuffer, HUD under the
 cabinet) on desktop; it combines with any of the above. `--layout=1`..`4`
 picks the portrait HUD arrangement (`PortraitLayout` in `src/draw.rs`: row,
 flank, queue strip, compact; compact is the default). Portrait leaves the
 controls legend off the game screen and shows it under the pause menu instead.

## iOS (iPhone, portrait only)

`ios/build.sh sim run` builds `aarch64-apple-ios-sim`, assembles
`target/ios/sim/Bloxide.app` from `ios/Info.plist`, and installs and launches
it on the simulator named by `BLOXIDE_SIMULATOR` (default `iPhone 17 Pro`).
Extra arguments are passed to the app: `ios/build.sh sim run --layout=3`.
`ios/build.sh device` needs `BLOXIDE_SIGN_IDENTITY` and `BLOXIDE_PROFILE`.
Screenshot the simulator with `xcrun simctl io "iPhone 17 Pro" screenshot out.png`.

- Controls are gestures (`src/touch.rs`): drag sideways to move, tap to rotate
 (right half clockwise, left half counter-clockwise), drag down to soft-drop,
 flick down to hard-drop, swipe up to hold. Multi-finger touches are ignored.
 The pause button beside the score lintel (`pause_button_rect` in
 `src/draw.rs`) toggles pause; a tap on it never rotates. Menus are tapped.
 Menus have no Quit item.
- The high score lives in the app's `Documents/.highscore`.
- Returning from the background pauses a running game.
- iOS renders through Metal (`render3d::USES_METAL`); everything else uses
 OpenGL. The postfx pass has an MSL twin of its GLSL shader, and the camera
 uses a shifted near plane so depth lands in Metal's 0..1 clip range.
- miniquad is vendored in `vendor/miniquad` (via `[patch.crates-io]`) because
 its Metal backend cannot draw into render targets as shipped. Each fix is
 marked `bloxide patch`. Run with `SIMCTL_CHILD_MTL_DEBUG_LAYER=1 xcrun simctl
 launch --console-pty <device> dev.bloxide.game` to surface Metal validation
 errors, which otherwise just drop draws silently.
- `cargo clippy --target aarch64-apple-ios-sim` should stay warning-free too.

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
- **Clippy**: `cargo clippy --all-targets` is warning-free; keep it that way.
- **Tests**: `cargo test` runs the unit suite (currently 179 tests) and needs no
  display.
- **High scores**: the game writes a `.highscore` file in the working
  directory. It is gitignored.
