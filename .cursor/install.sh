#!/usr/bin/env bash
# Idempotent setup for the bloxide development environment.
#
# bloxide renders through Macroquad (OpenGL via miniquad), so it needs the X11,
# OpenGL and ALSA development headers to build, plus a Rust toolchain new enough
# for its dependency tree. This script installs those and produces a debug
# build. It is safe to run repeatedly.
set -euo pipefail

sudo apt-get update
sudo apt-get install -y --no-install-recommends \
    libgl1-mesa-dev \
    libxi-dev \
    libasound2-dev \
    libxcursor-dev \
    libxrandr-dev \
    libxinerama-dev

# The pinned base toolchain predates `integer_sign_cast`, which fontdue (pulled
# in by Macroquad) uses, so build on current stable instead.
rustup toolchain install stable --profile minimal --no-self-update
rustup default stable

cargo build
