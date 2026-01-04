use std::fmt::Display;

use crate::piece::{pieces, Piece};
use rand::seq::SliceRandom;
use rand::thread_rng;

fn get_random_bag() -> [Piece; 7] {
    let mut rng = thread_rng();

    let mut bag = [
        pieces::I,
        pieces::J,
        pieces::L,
        pieces::O,
        pieces::S,
        pieces::T,
        pieces::Z,
    ];

    bag.shuffle(&mut rng);
    bag
}

#[derive(Copy, Clone, Debug)]
pub struct BagManager {
    index: isize,
    current_bag: [Piece; 7],
    next_bag: [Piece; 7],
}

impl BagManager {
    pub fn new() -> Self {
        Self {
            index: -1,
            current_bag: get_random_bag(),
            next_bag: get_random_bag(),
        }
    }

    pub fn next(&mut self) -> Piece {
        self.index += 1;

        if self.index > 6 {
            self.current_bag = self.next_bag;
            self.next_bag = get_random_bag();
            self.index = 0;
        }

        self.current_bag[self.index as usize]
    }

    pub fn peek(&self, offset: usize) -> Piece {
        let next_index = (self.index + offset as isize) as usize;

        if next_index > 6 {
            self.next_bag[next_index % 7]
        } else {
            self.current_bag[next_index]
        }
    }
}

impl Display for BagManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let current_bag_string = format!(
            "[{}]",
            self.current_bag
                .iter()
                .map(|piece| piece.to_string())
                .collect::<Vec<String>>()
                .join(",")
        );

        let next_bag_string = format!(
            "[{}]",
            self.next_bag
                .iter()
                .map(|piece| piece.to_string())
                .collect::<Vec<String>>()
                .join(",")
        );

        write!(
            f,
            "BagManager {{ index: {}, current_bag: {}, next_bag: {} }}",
            self.index, current_bag_string, next_bag_string
        )
    }
}
