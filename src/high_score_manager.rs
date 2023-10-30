use std::{cell::Cell, fs};

pub struct HighScoreManager {
    high_score: Cell<usize>,
}

impl HighScoreManager {
    pub fn new() -> Self {
        let read_high_score = fs::read_to_string("./.highscore")
            .and_then(|data| data.parse::<usize>().or(Ok(0)))
            .unwrap_or(0);

        Self {
            high_score: Cell::new(read_high_score),
        }
    }

    fn set_score(&self, score: usize) {
        self.high_score.set(score);
        fs::write("./.highscore", score.to_string()).expect("Unable to write file");
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
