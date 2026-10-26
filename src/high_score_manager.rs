use std::{cell::Cell, fs, path::PathBuf};

pub struct HighScoreManager {
    high_score: Cell<usize>,
}

/// iOS apps run from a read-only bundle, so the score lives in the app's
/// Documents folder there; elsewhere it stays in the working directory.
fn high_score_path() -> PathBuf {
    if cfg!(target_os = "ios") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join("Documents").join(".highscore");
        }
    }

    PathBuf::from("./.highscore")
}

impl HighScoreManager {
    pub fn new() -> Self {
        let read_high_score = fs::read_to_string(high_score_path())
            .and_then(|data| data.parse::<usize>().or(Ok(0)))
            .unwrap_or(0);

        Self {
            high_score: Cell::new(read_high_score),
        }
    }

    fn set_score(&self, score: usize) {
        self.high_score.set(score);

        if let Err(error) = fs::write(high_score_path(), score.to_string()) {
            eprintln!("Unable to save high score: {error}");
        }
    }

    pub fn add_score(&self, score: usize) {
        if score > self.high_score.get() {
            self.set_score(score)
        }
    }

    pub fn get_high_score(&self) -> usize {
        self.high_score.get()
    }
}
