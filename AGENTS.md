# Agents

## Cloud-specific instructions

### Overview

**bloxide** is a single-binary Tetris-style block puzzle game written in Rust using [Macroquad](https://macroquad.rs/) (OpenGL-based). No backend services, databases, or network dependencies.

### System dependencies (pre-installed on VM)

Macroquad requires X11/OpenGL libraries. These are installed via:

```
sudo apt-get install -y libxi-dev libgl1-mesa-dev libasound2-dev libxcursor-dev libxrandr-dev libxinerama-dev
```

### Standard commands

| Action | Command |
|--------|---------|
| Build | `cargo build` |
| Build (release) | `cargo build --release` |
| Lint | `cargo clippy` |
| Test | `cargo test` |
| Run | `DISPLAY=:1 cargo run` |

### Non-obvious caveats

- **Display required**: Macroquad needs an X display. The VM has Xvfb on `:1` — always set `DISPLAY=:1` when running the game.
- **Clippy warnings**: The existing codebase has ~12 clippy warnings (useless format, needless range loops, needless lifetimes, manual clamp, single match). These are pre-existing and not regressions.
- **No automated tests**: The project has zero unit/integration tests (`cargo test` passes with 0 tests). Validation is manual (run the game, play it).
- **High scores**: The game writes a `.highscore` file in the working directory. This is gitignored.
