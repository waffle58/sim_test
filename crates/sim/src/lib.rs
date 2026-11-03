pub use self::random::stable_random;
pub use self::simc::{Sim, POPULATION_CAP};

pub const TICKS_PER_SECOND: f64 = 20.0;

mod random;
mod simc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_population() {
        let mut a = Sim::new(7);
        let mut b = Sim::new(7);
        for _ in 0..1000 { a.tick(); b.tick(); }
        assert_eq!(a.population, b.population);
        assert_eq!(a.tick_count, b.tick_count);
    }

    #[test]
    fn population_never_decreases() {
        let mut s = Sim::new(7);
        let mut prev = s.population;
        for _ in 0..2000 {
            s.tick();
            assert!(s.population >= prev);
            prev = s.population;
        }
    }

    #[test]
    fn population_reaches_and_holds_at_cap() {
        let mut s = Sim::new(7);
        for _ in 0..10_000 { s.tick(); }
        assert_eq!(s.citizens(), POPULATION_CAP);
    }

    #[test]
    fn stable_random_is_stable() {
        assert_eq!(stable_random(1, 0, 9), stable_random(1, 0, 9));
        assert_ne!(stable_random(1, 0, 9), stable_random(2, 0, 9));
    }
}