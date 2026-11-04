//! lib.rs — module root for the whole simulation.

pub use self::random::stable_random;
pub use self::simc::{CitizenView, DeadCitizenView, Sim, DEFAULT_POPULATION_CAP};

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
        assert_eq!(a.citizen_eq, b.citizen_eq);
        assert_eq!(a.dead_citizen_ids, b.dead_citizen_ids);
        assert_eq!(a.food, b.food);
    }

    #[test]
    fn food_limits_growth() {
        // Starting food 100 / 10 per spawn = 10 more citizens possible.
        let mut s = Sim::new(7);
        for _ in 0..10_000 { s.tick(); }
        assert!(s.food <= 0, "food should be exhausted, got {}", s.food);
        // 10 starting + up to 10 from food, minus any deaths
        assert!(s.citizens() <= 20, "grew past food budget: {}", s.citizens());
    }

    #[test]
    fn adding_food_enables_more_growth() {
        let mut s = Sim::new(7);
        for _ in 0..2000 { s.tick(); }
        let before = s.citizens();
        s.add_food(10_000);
        for _ in 0..2000 { s.tick(); }
        assert!(s.citizens() >= before, "population shrank unexpectedly");
        assert!(s.citizens() + s.dead_count() > before || s.citizens() > before,
                "no new citizens appeared after adding food");
    }

    #[test]
    fn death_moves_to_dead_columns() {
        let mut s = Sim::new(7);
        s.add_food(10_000);
        // Run long enough that at least one death fires (interval 100-300).
        for _ in 0..2000 { s.tick(); }
        assert!(s.dead_count() > 0, "no deaths fired in 2000 ticks");
        // No ID appears in both alive and dead.
        let alive: std::collections::HashSet<_> = s.citizen_ids.iter().copied().collect();
        for &id in &s.dead_citizen_ids {
            assert!(!alive.contains(&id), "id {} is both alive and dead", id);
        }
    }

    #[test]
    fn save_load_preserves_everything() {
        let mut a = Sim::new(7);
        a.add_food(10_000);
        a.set_cap(100);
        for _ in 0..2000 { a.tick(); }
        assert!(a.dead_count() > 0, "test needs at least one death");

        let path = std::env::temp_dir().join("city_sim_test_save");
        let _ = std::fs::remove_dir_all(&path);
        let path = path.to_str().unwrap();
        a.save(path).unwrap();

        let mut b = Sim::load(path).unwrap();
        assert_eq!(a.food, b.food);
        assert_eq!(a.population_cap, b.population_cap);
        assert_eq!(a.citizen_ids, b.citizen_ids);
        assert_eq!(a.citizen_eq, b.citizen_eq);
        assert_eq!(a.dead_citizen_ids, b.dead_citizen_ids);
        assert_eq!(a.dead_citizen_eq, b.dead_citizen_eq);
        assert_eq!(a.dead_citizen_death_tick, b.dead_citizen_death_tick);

        for _ in 0..200 { a.tick(); b.tick(); }
        assert_eq!(a.citizen_eq, b.citizen_eq);
        assert_eq!(a.food, b.food);
    }

    #[test]
    fn citizen_codes_are_unique() {
        let s = Sim::new(7);
        let mut seen = std::collections::HashSet::new();
        for i in 0..s.citizens() as usize {
            assert!(seen.insert(s.citizen_code(i)));
        }
    }
}