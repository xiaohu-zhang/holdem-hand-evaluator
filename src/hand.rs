use assets::constants::*;
use assets::lookup::{LOOKUP, LOOKUP_FLUSH};
use assets::offsets::OFFSETS;
use std::ops::{Add, AddAssign};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HandCategory {
    HighCard = 0,
    OnePair = 1,
    TwoPair = 2,
    ThreeOfAKind = 3,
    Straight = 4,
    Flush = 5,
    FullHouse = 6,
    FourOfAKind = 7,
    StraightFlush = 8,
}

/// Returns the hand category from hand rank computed by `Hand::evaluate()`.
#[inline]
pub fn get_hand_category(hand_rank: u16) -> HandCategory {
    match hand_rank >> 12 {
        0 => HandCategory::HighCard,
        1 => HandCategory::OnePair,
        2 => HandCategory::TwoPair,
        3 => HandCategory::ThreeOfAKind,
        4 => HandCategory::Straight,
        5 => HandCategory::Flush,
        6 => HandCategory::FullHouse,
        7 => HandCategory::FourOfAKind,
        8 => HandCategory::StraightFlush,
        _ => unreachable!(),
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Hand {
    key: u64,
    mask: u64,
}

impl Hand {
    /// Creates an empty `Hand` struct.
    #[inline]
    pub fn new() -> Self {
        Self {
            key: 0x3333 << SUIT_SHIFT,
            mask: 0,
        }
    }

    /// Creates a new hand structure consists of `cards`.
    /// Elements in `cards` must be in the range \[0, 51\].
    /// (0 corresponds to the deuce of clubs, and 51 corresponds to the ace of spades)
    #[inline]
    pub fn from_slice(cards: &[usize]) -> Self {
        let mut hand = Self::new();
        for card in cards {
            hand = hand.add_card(*card);
        }
        hand
    }

    /// Checks whether the hand is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.mask == 0
    }

    /// Returns current number of cards in `self`.
    #[inline]
    pub fn len(&self) -> usize {
        self.mask.count_ones() as usize
    }

    /// Returns the bit mask of `self`.
    #[inline]
    pub fn get_mask(&self) -> u64 {
        self.mask
    }

    /// Returns whether the `card` is included in `self`.
    #[inline]
    pub fn contains(&self, card: usize) -> bool {
        (self.mask & unsafe { *CARDS.get_unchecked(card) }.1) != 0
    }

    /// Returns a new hand struct where `card` is added to `self`.
    /// `card` must be in the range \[0, 51\] and must not be already included in `self`.
    /// (0 corresponds to the deuce of clubs, and 51 corresponds to the ace of spades)
    #[inline]
    pub fn add_card(&self, card: usize) -> Self {
        let (k, m) = unsafe { *CARDS.get_unchecked(card) };
        Self {
            key: self.key.wrapping_add(k),
            mask: self.mask.wrapping_add(m),
        }
    }

    /// Returns a new hand struct where `card` is removed from `self`.
    /// `card` must be in the range \[0, 51\] and included in `self`.
    #[inline]
    pub fn remove_card(&self, card: usize) -> Self {
        let (k, m) = unsafe { *CARDS.get_unchecked(card) };
        Self {
            key: self.key.wrapping_sub(k),
            mask: self.mask.wrapping_sub(m),
        }
    }

    /// Returns hand strength in 16-bit integer.
    /// This function may crush when `self.len() < 5 || self.len() > 7`.
    #[inline]
    pub fn evaluate(&self) -> u16 {
        let is_flush = self.key & FLUSH_MASK;
        if is_flush > 0 {
            let flush_key = (self.mask >> (4 * is_flush.leading_zeros())) as u16;
            unsafe { *LOOKUP_FLUSH.get_unchecked(flush_key as usize) }
        } else {
            let rank_key = self.key as u32 as usize;
            let offset = unsafe { *OFFSETS.get_unchecked(rank_key >> OFFSET_SHIFT) as usize };
            let hash_key = rank_key.wrapping_add(offset);
            unsafe { *LOOKUP.get_unchecked(hash_key) }
        }
    }
}

impl Add for Hand {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            key: self
                .key
                .wrapping_add(rhs.key)
                .wrapping_sub(0x3333 << SUIT_SHIFT),
            mask: self.mask.wrapping_add(rhs.mask),
        }
    }
}

impl AddAssign for Hand {
    fn add_assign(&mut self, rhs: Self) {
        self.key = self.key.wrapping_add(rhs.key);
        self.key = self.key.wrapping_sub(0x3333 << SUIT_SHIFT);
        self.mask = self.mask.wrapping_add(rhs.mask);
    }
}

impl Default for Hand {
    fn default() -> Self {
        Self::new()
    }
}

impl FromStr for Hand {
    type Err = String;

    fn from_str(hand_str: &str) -> Result<Self, Self::Err> {
        let mut hand = Self::new();
        let mut chars = hand_str.chars();
        loop {
            let rank_opt = chars.next();
            if rank_opt.is_none() {
                return Ok(hand);
            }
            let rank_char = rank_opt.unwrap();
            let suit_char = chars
                .next()
                .ok_or("parse failed: expected suit character, but got EOF")?;
            let rank_id = match rank_char.to_ascii_uppercase() {
                '2' => Ok(0),
                '3' => Ok(1),
                '4' => Ok(2),
                '5' => Ok(3),
                '6' => Ok(4),
                '7' => Ok(5),
                '8' => Ok(6),
                '9' => Ok(7),
                'T' => Ok(8),
                'J' => Ok(9),
                'Q' => Ok(10),
                'K' => Ok(11),
                'A' => Ok(12),
                ch => Err(format!(
                    "parse failed: expected rank character, but got '{}'",
                    ch
                )),
            }?;
            let suit_id = match suit_char.to_ascii_lowercase() {
                's' => Ok(0),
                'h' => Ok(1),
                'c' => Ok(2),
                'd' => Ok(3),
                ch => Err(format!(
                    "parse failed: expected suit character, but got '{}'",
                    ch
                )),
            }?;
            hand = hand.add_card(rank_id * 4 + suit_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn evaluate_hand_str(hand_str: &str) -> u16 {
        let hand = hand_str.parse::<Hand>().unwrap();
        assert_eq!(hand.len(), 7);
        hand.evaluate()
    }

    #[test]
    fn test_parser() {
        let cards = [2, 3, 5, 7, 11, 13, 17];
        let hand_from_vec = Hand::from_slice(&cards);
        let hand_from_str = "2h2s3d3s4s5d6d".parse::<Hand>();
        assert_eq!(hand_from_str, Ok(hand_from_vec));
        assert_eq!("".parse::<Hand>(), Ok(Hand::new()));
        assert_eq!(
            "A".parse::<Hand>(),
            Err("parse failed: expected suit character, but got EOF".into())
        );
        assert_eq!(
            "Ax".parse::<Hand>(),
            Err("parse failed: expected suit character, but got 'x'".into())
        );
        assert_eq!(
            "10s".parse::<Hand>(),
            Err("parse failed: expected rank character, but got '1'".into())
        );
    }

    #[test]
    fn test_all_5card_combinations() {
        let mut rankset = HashSet::new();
        let mut counter = vec![0; HandCategory::StraightFlush as usize + 1];

        for i in 0..(NUMBER_OF_CARDS - 4) {
            let hand = Hand::new().add_card(i);
            for j in (i + 1)..(NUMBER_OF_CARDS - 3) {
                let hand = hand.add_card(j);
                for k in (j + 1)..(NUMBER_OF_CARDS - 2) {
                    let hand = hand.add_card(k);
                    for m in (k + 1)..(NUMBER_OF_CARDS - 1) {
                        let hand = hand.add_card(m);
                        for n in (m + 1)..NUMBER_OF_CARDS {
                            let hand = hand.add_card(n);
                            let rank = hand.evaluate();
                            let category = get_hand_category(rank);
                            rankset.insert(rank);
                            counter[category as usize] += 1;
                        }
                    }
                }
            }
        }

        assert_eq!(rankset.len(), 7462);
        assert_eq!(counter[HandCategory::StraightFlush as usize], 40);
        assert_eq!(counter[HandCategory::FourOfAKind as usize], 624);
        assert_eq!(counter[HandCategory::FullHouse as usize], 3744);
        assert_eq!(counter[HandCategory::Flush as usize], 5108);
        assert_eq!(counter[HandCategory::Straight as usize], 10200);
        assert_eq!(counter[HandCategory::ThreeOfAKind as usize], 54912);
        assert_eq!(counter[HandCategory::TwoPair as usize], 123552);
        assert_eq!(counter[HandCategory::OnePair as usize], 1098240);
        assert_eq!(counter[HandCategory::HighCard as usize], 1302540);
    }

    #[test]
    fn test_all_6card_combinations() {
        let mut rankset = HashSet::new();
        let mut counter = vec![0; HandCategory::StraightFlush as usize + 1];

        for i in 0..(NUMBER_OF_CARDS - 5) {
            let hand = Hand::new().add_card(i);
            for j in (i + 1)..(NUMBER_OF_CARDS - 4) {
                let hand = hand.add_card(j);
                for k in (j + 1)..(NUMBER_OF_CARDS - 3) {
                    let hand = hand.add_card(k);
                    for m in (k + 1)..(NUMBER_OF_CARDS - 2) {
                        let hand = hand.add_card(m);
                        for n in (m + 1)..(NUMBER_OF_CARDS - 1) {
                            let hand = hand.add_card(n);
                            for p in (n + 1)..NUMBER_OF_CARDS {
                                let hand = hand.add_card(p);
                                let rank = hand.evaluate();
                                let category = get_hand_category(rank);
                                rankset.insert(rank);
                                counter[category as usize] += 1;
                            }
                        }
                    }
                }
            }
        }

        assert_eq!(rankset.len(), 6075);
        assert_eq!(counter[HandCategory::StraightFlush as usize], 1844);
        assert_eq!(counter[HandCategory::FourOfAKind as usize], 14664);
        assert_eq!(counter[HandCategory::FullHouse as usize], 165984);
        assert_eq!(counter[HandCategory::Flush as usize], 205792);
        assert_eq!(counter[HandCategory::Straight as usize], 361620);
        assert_eq!(counter[HandCategory::ThreeOfAKind as usize], 732160);
        assert_eq!(counter[HandCategory::TwoPair as usize], 2532816);
        assert_eq!(counter[HandCategory::OnePair as usize], 9730740);
        assert_eq!(counter[HandCategory::HighCard as usize], 6612900);
    }

    #[test]
    fn test_all_7card_combinations() {
        let mut rankset = HashSet::new();
        let mut counter = vec![0; HandCategory::StraightFlush as usize + 1];

        for i in 0..(NUMBER_OF_CARDS - 6) {
            let hand = Hand::new().add_card(i);
            for j in (i + 1)..(NUMBER_OF_CARDS - 5) {
                let hand = hand.add_card(j);
                for k in (j + 1)..(NUMBER_OF_CARDS - 4) {
                    let hand = hand.add_card(k);
                    for m in (k + 1)..(NUMBER_OF_CARDS - 3) {
                        let hand = hand.add_card(m);
                        for n in (m + 1)..(NUMBER_OF_CARDS - 2) {
                            let hand = hand.add_card(n);
                            for p in (n + 1)..(NUMBER_OF_CARDS - 1) {
                                let hand = hand.add_card(p);
                                for q in (p + 1)..NUMBER_OF_CARDS {
                                    let hand = hand.add_card(q);
                                    let rank = hand.evaluate();
                                    let category = get_hand_category(rank);
                                    rankset.insert(rank);
                                    counter[category as usize] += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        assert_eq!(rankset.len(), 4824);
        assert_eq!(counter[HandCategory::StraightFlush as usize], 41584);
        assert_eq!(counter[HandCategory::FourOfAKind as usize], 224848);
        assert_eq!(counter[HandCategory::FullHouse as usize], 3473184);
        assert_eq!(counter[HandCategory::Flush as usize], 4047644);
        assert_eq!(counter[HandCategory::Straight as usize], 6180020);
        assert_eq!(counter[HandCategory::ThreeOfAKind as usize], 6461620);
        assert_eq!(counter[HandCategory::TwoPair as usize], 31433400);
        assert_eq!(counter[HandCategory::OnePair as usize], 58627800);
        assert_eq!(counter[HandCategory::HighCard as usize], 23294460);
    }

    #[test]
    fn test_edge_cases() {
        // straight flushes
        assert_eq!(evaluate_hand_str("AsKsQsJsTs7d5s"), (8 << 12) + 9);
        assert_eq!(evaluate_hand_str("Ac7c6c5c4c3c2c"), (8 << 12) + 2);
        assert_eq!(evaluate_hand_str("AdQsJc5d4d3d2d"), (8 << 12) + 0);

        // four of a kinds
        assert_eq!(evaluate_hand_str("AsAcAhAdKsQcTh"), (7 << 12) + 155);
        assert_eq!(evaluate_hand_str("3d3h3s2c2d2h2s"), (7 << 12) + 0);

        // full houses
        assert_eq!(evaluate_hand_str("AsAdAhKcKdKh2d"), (6 << 12) + 155);
        assert_eq!(evaluate_hand_str("4h4c3s3c2d2c2h"), (6 << 12) + 1);
        assert_eq!(evaluate_hand_str("5h4c3s3c2d2c2h"), (6 << 12) + 0);

        // flushes
        assert_eq!(evaluate_hand_str("AhKhQhJh9h9c9s"), (5 << 12) + 1276);
        assert_eq!(evaluate_hand_str("Js7c6d5c4c3c2c"), (5 << 12) + 0);

        // straights
        assert_eq!(evaluate_hand_str("AhKcKdKhQcJdTs"), (4 << 12) + 9);
        assert_eq!(evaluate_hand_str("Ac8c7c5d4d3d2d"), (4 << 12) + 0);

        // three of a kinds
        assert_eq!(evaluate_hand_str("AsAcAhKhQd5c3s"), (3 << 12) + 857);
        assert_eq!(evaluate_hand_str("7d5c4c3c2d2s2h"), (3 << 12) + 8);

        // two pairs
        assert_eq!(evaluate_hand_str("AsAhKsKhQsQhJs"), (2 << 12) + 857);
        assert_eq!(evaluate_hand_str("7c6d5h3s3c2d2h"), (2 << 12) + 3);

        // one pairs
        assert_eq!(evaluate_hand_str("AdAsKhQdJs3s2c"), (1 << 12) + 2859);
        assert_eq!(evaluate_hand_str("8s7s5h4c3c2d2c"), (1 << 12) + 18);

        // high cards
        assert_eq!(evaluate_hand_str("AdKdQdJd9s3h2c"), (0 << 12) + 1276);
        assert_eq!(evaluate_hand_str("9h8s7d5d4d3c2d"), (0 << 12) + 48);
    }

    #[test]
    fn test_hand_addition() {
        let hand1 = "4h4c".parse::<Hand>().unwrap();
        let hand2 = "5h4s".parse::<Hand>().unwrap();
        let board = "3s3c2d2c2h".parse::<Hand>().unwrap();
        assert_eq!((hand1 + board).evaluate(), (6 << 12) + 1);
        assert_eq!((hand2 + board).evaluate(), (6 << 12) + 0);
    }
}
