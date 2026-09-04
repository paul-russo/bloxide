//! Deterministic scoring for already-classified locks.
//!
//! Uses the common modern tables: Mini Doubles are supported, difficult clears
//! chain back-to-back, and consecutive clearing locks build a combo. Drop points
//! are awarded separately by gameplay; perfect-clear bonuses are not included.
//! Reference: https://tetris.wiki/Scoring

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpinKind {
    #[default]
    None,
    Mini,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LockEvent {
    pub spin: SpinKind,
    pub lines: usize,
    pub level: usize,
}

impl LockEvent {
    fn base_points(self) -> usize {
        let points = match (self.spin, self.lines) {
            (SpinKind::None, 0) => 0,
            (SpinKind::None, 1) => 100,
            (SpinKind::None, 2) => 300,
            (SpinKind::None, 3) => 500,
            (SpinKind::None, 4) => 800,
            (SpinKind::Mini, 0) => 100,
            (SpinKind::Mini, 1) => 200,
            (SpinKind::Mini, 2) => 400,
            (SpinKind::Full, 0) => 400,
            (SpinKind::Full, 1) => 800,
            (SpinKind::Full, 2) => 1200,
            (SpinKind::Full, 3) => 1600,
            _ => panic!("invalid spin/line-count combination: {self:?}"),
        };

        points * self.level
    }

    fn is_difficult(self) -> bool {
        self.lines > 0 && (self.lines == 4 || self.spin != SpinKind::None)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScoringState {
    back_to_back: bool,
    combo: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoreAward {
    pub event: LockEvent,
    pub base: usize,
    pub back_to_back_bonus: usize,
    pub combo_bonus: usize,
    /// The first clearing lock is combo zero; a non-clearing lock has no combo.
    pub combo: Option<usize>,
}

impl ScoreAward {
    pub fn total(self) -> usize {
        self.base + self.back_to_back_bonus + self.combo_bonus
    }
}

/// Score one lock using its pre-clear level, without consulting the board,
/// clock, input, or rendering. A no-line lock breaks the combo but preserves B2B.
pub fn score_lock(event: LockEvent, previous: ScoringState) -> (ScoringState, ScoreAward) {
    assert!(event.level > 0, "scoring levels start at one");

    let base = event.base_points();
    let difficult = event.is_difficult();
    let back_to_back_bonus = if difficult && previous.back_to_back {
        base / 2
    } else {
        0
    };
    let combo = if event.lines > 0 {
        Some(previous.combo.map_or(0, |count| count + 1))
    } else {
        None
    };
    let combo_bonus = 50 * combo.unwrap_or(0) * event.level;
    let next = ScoringState {
        back_to_back: if event.lines > 0 {
            difficult
        } else {
            previous.back_to_back
        },
        combo,
    };
    let award = ScoreAward {
        event,
        base,
        back_to_back_bonus,
        combo_bonus,
        combo,
    };

    (next, award)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(spin: SpinKind, lines: usize, level: usize) -> LockEvent {
        LockEvent { spin, lines, level }
    }

    #[test]
    fn ordinary_clear_table_uses_the_supplied_level() {
        for level in [1, 2, 20] {
            for (lines, base) in [(0, 0), (1, 100), (2, 300), (3, 500), (4, 800)] {
                let lock = event(SpinKind::None, lines, level);
                let (state, award) = score_lock(lock, ScoringState::default());

                assert_eq!(award.event, lock);
                assert_eq!(award.base, base * level);
                assert_eq!(award.total(), base * level);
                assert_eq!(award.combo, (lines > 0).then_some(0));
                assert_eq!(state.combo, award.combo);
                assert_eq!(state.back_to_back, lines == 4);
            }
        }
    }

    #[test]
    fn full_and_mini_spin_tables_include_zero_lines_and_mini_doubles() {
        for level in [1, 3, 20] {
            for (spin, lines, base) in [
                (SpinKind::Mini, 0, 100),
                (SpinKind::Mini, 1, 200),
                (SpinKind::Mini, 2, 400),
                (SpinKind::Full, 0, 400),
                (SpinKind::Full, 1, 800),
                (SpinKind::Full, 2, 1200),
                (SpinKind::Full, 3, 1600),
            ] {
                let (state, award) = score_lock(event(spin, lines, level), ScoringState::default());

                assert_eq!(award.base, base * level);
                assert_eq!(award.total(), base * level);
                assert_eq!(award.back_to_back_bonus, 0);
                assert_eq!(award.combo_bonus, 0);
                assert_eq!(state.back_to_back, lines > 0);
            }
        }
    }

    #[test]
    fn every_difficult_clear_can_continue_back_to_back() {
        let (previous, _) = score_lock(event(SpinKind::None, 4, 1), ScoringState::default());

        for (spin, lines, base) in [
            (SpinKind::None, 4, 800),
            (SpinKind::Mini, 1, 200),
            (SpinKind::Mini, 2, 400),
            (SpinKind::Full, 1, 800),
            (SpinKind::Full, 2, 1200),
            (SpinKind::Full, 3, 1600),
        ] {
            let (state, award) = score_lock(event(spin, lines, 1), previous);

            assert!(state.back_to_back);
            assert_eq!(award.base, base);
            assert_eq!(award.back_to_back_bonus, base / 2);
            assert_eq!(award.combo, Some(1));
            assert_eq!(award.combo_bonus, 50);
            assert_eq!(award.total(), base + base / 2 + 50);
        }
    }

    #[test]
    fn nonclearing_locks_preserve_back_to_back_but_reset_the_combo() {
        for spin in [SpinKind::None, SpinKind::Mini, SpinKind::Full] {
            let (previous, _) = score_lock(event(SpinKind::None, 4, 1), ScoringState::default());
            let (between, award) = score_lock(event(spin, 0, 1), previous);

            assert!(between.back_to_back);
            assert_eq!(between.combo, None);
            assert_eq!(award.back_to_back_bonus, 0);
            assert_eq!(award.combo_bonus, 0);

            let (state, next) = score_lock(event(SpinKind::None, 4, 1), between);

            assert!(state.back_to_back);
            assert_eq!(next.combo, Some(0));
            assert_eq!(next.back_to_back_bonus, 400);
            assert_eq!(next.total(), 1200);
        }
    }

    #[test]
    fn ordinary_clears_break_back_to_back_without_breaking_a_combo() {
        for lines in 1..=3 {
            let (first, _) = score_lock(event(SpinKind::None, 4, 1), ScoringState::default());
            let (between, award) = score_lock(event(SpinKind::None, lines, 1), first);

            assert!(!between.back_to_back);
            assert_eq!(award.back_to_back_bonus, 0);
            assert_eq!(award.combo_bonus, 50);

            let (state, next) = score_lock(event(SpinKind::None, 4, 1), between);

            assert!(state.back_to_back);
            assert_eq!(next.back_to_back_bonus, 0);
            assert_eq!(next.combo, Some(2));
            assert_eq!(next.combo_bonus, 100);
            assert_eq!(next.total(), 900);
        }
    }

    #[test]
    fn zero_line_spins_cannot_start_or_receive_a_back_to_back_bonus() {
        for spin in [SpinKind::Mini, SpinKind::Full] {
            let (state, award) = score_lock(event(spin, 0, 1), ScoringState::default());

            assert!(!state.back_to_back);
            assert_eq!(state.combo, None);
            assert_eq!(award.back_to_back_bonus, 0);

            let (state, first_clear) = score_lock(event(SpinKind::None, 4, 1), state);
            let (state, zero_spin) = score_lock(event(spin, 0, 1), state);

            assert_eq!(first_clear.total(), 800);
            assert!(state.back_to_back);
            assert_eq!(zero_spin.back_to_back_bonus, 0);
            assert_eq!(zero_spin.combo_bonus, 0);
        }
    }

    #[test]
    fn combo_starts_at_zero_and_restarts_after_any_nonclearing_lock() {
        for breaker in [SpinKind::None, SpinKind::Mini, SpinKind::Full] {
            let mut state = ScoringState::default();

            for index in 0..4 {
                let (next, award) = score_lock(event(SpinKind::None, 1, 1), state);
                state = next;

                assert_eq!(award.combo, Some(index));
                assert_eq!(award.combo_bonus, index * 50);
                assert_eq!(award.total(), 100 + index * 50);
            }

            let (state, _) = score_lock(event(breaker, 0, 1), state);
            let (_, award) = score_lock(event(SpinKind::None, 1, 1), state);

            assert_eq!(award.combo, Some(0));
            assert_eq!(award.total(), 100);
        }
    }

    #[test]
    fn back_to_back_multiplies_only_the_base_not_the_combo() {
        let previous = ScoringState {
            back_to_back: true,
            combo: Some(2),
        };
        let (_, award) = score_lock(event(SpinKind::Full, 2, 3), previous);

        assert_eq!(award.base, 3600);
        assert_eq!(award.back_to_back_bonus, 1800);
        assert_eq!(award.combo_bonus, 450);
        assert_eq!(award.total(), 5850);
    }

    #[test]
    fn two_consecutive_level_one_tetrises_award_eight_hundred_then_twelve_fifty() {
        let (state, first) = score_lock(event(SpinKind::None, 4, 1), ScoringState::default());
        let (_, second) = score_lock(event(SpinKind::None, 4, 1), state);

        assert_eq!(first.total(), 800);
        assert_eq!(second.total(), 1250);
    }

    #[test]
    #[should_panic(expected = "invalid spin/line-count combination")]
    fn impossible_mini_triples_are_rejected() {
        score_lock(event(SpinKind::Mini, 3, 1), ScoringState::default());
    }

    #[test]
    #[should_panic(expected = "scoring levels start at one")]
    fn a_zero_level_is_rejected() {
        score_lock(event(SpinKind::None, 1, 0), ScoringState::default());
    }
}
