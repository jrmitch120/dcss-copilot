//! Crawl's random numbers, ported from `rng::PcgRNG` in `pcg.cc` and the helpers in `random.cc`
//! (crawl 0.34.1), so ported formulas roll dice exactly as crawl does.
//!
//! The generator is seeded by the engine, never from the clock, so the same Game Situation
//! always gives the same Fact Sheet.

/// A PCG32 generator: `rng::PcgRNG` in crawl's `pcg.cc`, after O'Neill's minimal PCG.
#[derive(Debug, Clone)]
pub(crate) struct Rng {
    state: u64,
    inc: u64,
}

/// PCG's multiplier, as `PcgRNG::get_uint32` uses it.
const PCG_MULTIPLIER: u64 = 6_364_136_223_846_793_005;

impl Rng {
    /// A generator seeded as `PcgRNG(init_state, sequence)` seeds one.
    pub(crate) fn new(init_state: u64, sequence: u64) -> Self {
        let mut rng = Self {
            state: 0,
            inc: (sequence << 1) | 1,
        };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(init_state);
        rng.next_u32();
        rng
    }

    /// `PcgRNG::get_uint32`: a uniformly distributed 32-bit number.
    fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(PCG_MULTIPLIER).wrapping_add(self.inc | 1);
        // Truncation is the XSH RR output function.
        #[allow(clippy::cast_possible_truncation)]
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        #[allow(clippy::cast_possible_truncation)]
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// `PcgRNG::get_bounded_uint32`: uniform in `0..range`, by Lemire's method.
    fn bounded(&mut self, range: u32) -> u32 {
        let mut m = u64::from(self.next_u32()) * u64::from(range);
        #[allow(clippy::cast_possible_truncation)]
        let mut low = m as u32;
        if low < range {
            let threshold = range.wrapping_neg() % range;
            while low < threshold {
                m = u64::from(self.next_u32()) * u64::from(range);
                #[allow(clippy::cast_possible_truncation)]
                {
                    low = m as u32;
                }
            }
        }
        #[allow(clippy::cast_possible_truncation)]
        let high = (m >> 32) as u32;
        high
    }

    /// `random2(max)` (`random.cc:211`): uniform in `0..max`, and 0 when `max` is at most 1.
    pub(crate) fn random2(&mut self, max: i32) -> i32 {
        match u32::try_from(max) {
            Ok(range) if range > 1 => i32::try_from(self.bounded(range)).unwrap_or(0),
            _ => 0,
        }
    }

    /// `random2avg(max, rolls)` (`random.cc:339`): the mean of `random2(max)` with less variance.
    pub(crate) fn random2avg(&mut self, max: i32, rolls: i32) -> i32 {
        let mut sum = self.random2(max);
        for _ in 0..rolls - 1 {
            sum += self.random2(max + 1);
        }
        sum / rolls
    }

    /// `div_rand_round(num, den)` (`random.cc:313`): `num / den`, rounded up with the chance of
    /// the remainder.
    pub(crate) fn div_rand_round(&mut self, num: i32, den: i32) -> i32 {
        let rem = num % den;
        if rem == 0 {
            num / den
        } else {
            num / den + i32::from(self.random2(den) < rem)
        }
    }

    /// `x_chance_in_y(x, y)` (`random.cc:435`).
    pub(crate) fn x_chance_in_y(&mut self, x: i32, y: i32) -> bool {
        if x <= 0 {
            return false;
        }
        if x >= y {
            return true;
        }
        self.random2(y) < x
    }

    /// `roll_dice(num, size)` (`random.cc:266`): the sum of `num` dice numbered 1 to `size`.
    pub(crate) fn roll_dice(&mut self, num: i32, size: i32) -> i32 {
        if num <= 0 || size <= 0 {
            return 0;
        }
        let mut total = num;
        for _ in 0..num {
            total += self.random2(size);
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How often each value of `roll` came up in `trials` rolls, by value.
    fn frequencies(trials: u32, mut roll: impl FnMut(&mut Rng) -> i32) -> Vec<f64> {
        let mut rng = Rng::new(42, 54);
        let mut counts = Vec::new();
        for _ in 0..trials {
            let value = usize::try_from(roll(&mut rng)).unwrap();
            if counts.len() <= value {
                counts.resize(value + 1, 0_u32);
            }
            counts[value] += 1;
        }
        counts
            .into_iter()
            .map(|count| f64::from(count) / f64::from(trials))
            .collect()
    }

    fn assert_close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len(), "{actual:?} vs {expected:?}");
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 0.005, "{actual:?} vs {expected:?}");
        }
    }

    #[test]
    fn pcg_matches_the_reference_output_of_pcg32() {
        // pcg32-demo from pcg-c-basic, seeded (42, 54): its first outputs.
        let mut rng = Rng::new(42, 54);
        let outputs: Vec<u32> = (0..6).map(|_| rng.next_u32()).collect();
        assert_eq!(
            outputs,
            [
                0xa15c_02b7,
                0x7b47_f409,
                0xba1d_3330,
                0x83d2_f293,
                0xbfa4_784b,
                0xcbed_606e
            ]
        );
    }

    #[test]
    fn random2_is_uniform_below_its_bound() {
        assert_close(&frequencies(200_000, |rng| rng.random2(4)), &[0.25; 4]);
    }

    #[test]
    fn random2_of_one_or_less_is_zero() {
        let mut rng = Rng::new(1, 1);
        assert_eq!(rng.random2(1), 0);
        assert_eq!(rng.random2(0), 0);
        assert_eq!(rng.random2(-3), 0);
    }

    #[test]
    fn random2avg_averages_one_roll_below_max_with_the_rest_up_to_max() {
        // random2avg(4, 2) = (random2(4) + random2(5)) / 2, worked by hand over the 20 pairs:
        // sums 0..=7 occur 1,2,3,4,4,3,2,1 times, halved and floored to 0,1,2,3.
        assert_close(
            &frequencies(400_000, |rng| rng.random2avg(4, 2)),
            &[3.0 / 20.0, 7.0 / 20.0, 7.0 / 20.0, 3.0 / 20.0],
        );
    }

    #[test]
    fn div_rand_round_rounds_up_with_the_chance_of_the_remainder() {
        assert_close(
            &frequencies(200_000, |rng| rng.div_rand_round(7, 4)),
            &[0.0, 0.25, 0.75],
        );
    }

    #[test]
    fn roll_dice_sums_dice_from_one() {
        assert_close(
            &frequencies(200_000, |rng| rng.roll_dice(1, 4)),
            &[0.0, 0.25, 0.25, 0.25, 0.25],
        );
    }
}
