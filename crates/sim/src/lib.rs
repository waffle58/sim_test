pub use self::random::stable_random;
pub use self::simc::{CitizenView, Sim, POPULATION_CAP};

pub const TICKS_PER_SECOND: f64 = 20.0;

mod random;
mod simc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_citizens() {
        let mut a = Sim::new(7);
        let mut b = Sim::new(7);
        for _ in 0..500 { a.tick(); b.tick(); }
        assert_eq!(a.citizen_ids, b.citizen_ids);
        assert_eq!(a.citizen_capability, b.citizen_capability);
        assert_eq!(a.citizen_next_decision, b.citizen_next_decision);
    }

    #[test]
    fn citizen_count_reaches_cap_and_holds() {
        let mut s = Sim::new(7);
        for _ in 0..10_000 { s.tick(); }
        assert_eq!(s.citizens(), POPULATION_CAP);
    }

    #[test]
    fn citizens_are_distinct() {
        let s = Sim::new(7);
        assert_eq!(s.citizens(), 10);
        let caps: Vec<f32> =
            (0..s.citizens() as usize).map(|i| s.citizen(i).capability).collect();
        let min = caps.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = caps.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(max - min > 0.5, "capabilities clustered: {} .. {}", min, max);
    }

    #[test]
    fn save_load_preserves_citizens() {
        let mut a = Sim::new(7);
        for _ in 0..200 { a.tick(); }
        let path = std::env::temp_dir().join("citizen_test.save");
        let path = path.to_str().unwrap();
        a.save(path).unwrap();

        let mut b = Sim::load(path).unwrap();
        assert_eq!(a.citizen_ids, b.citizen_ids);
        assert_eq!(a.citizen_capability, b.citizen_capability);
        assert_eq!(a.citizen_next_decision, b.citizen_next_decision);
        assert_eq!(a.next_citizen_id, b.next_citizen_id);

        // And they stay in sync after further ticks.
        for _ in 0..100 { a.tick(); b.tick(); }
        assert_eq!(a.citizen_capability, b.citizen_capability);
    }
}