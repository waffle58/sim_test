//mod names;
mod random;
mod save;
mod sim;
mod tunables;

pub use self::random::stable_random;
pub use self::sim::{Civ, DeadCiv, Sim};
pub use self::tunables::Tunables;


pub const TICKS_PER_SECOND: f64 = 20.0;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_citizens() {
        let mut a = Sim::new(7);
        let mut b = Sim::new(7);
        for _ in 0..500 {
            a.tick();
            b.tick();
        }
        assert_eq!(a.civ_ids, b.civ_ids);
        assert_eq!(a.civ_eq, b.civ_eq);
        assert_eq!(a.dead_civ_ids, b.dead_civ_ids);
        assert_eq!(a.food, b.food);
    }

    #[test]
    fn food_limits_growth() {
        // Starting food 100 / 10 per spawn = 10 more citizens possible.
        let mut s = Sim::new(7);
        for _ in 0..10_000 {
            s.tick();
        }
        assert!(s.food <= 0, "food should be exhausted, got {}", s.food);
        // 10 starting + up to 10 from food, minus any deaths
        assert!(
            s.civs() <= 20,
            "grew past food budget: {}",
            s.civs()
        );
    }

    #[test]
    fn adding_food_enables_more_growth() {
        let mut s = Sim::new(7);
        for _ in 0..2000 {
            s.tick();
        }
        let before = s.civs();
        s.add_food(10_000);
        for _ in 0..2000 {
            s.tick();
        }
        assert!(s.civs() >= before, "population shrank unexpectedly");
        assert!(
            s.civs() + s.dead_civs() > before || s.civs() > before,
            "no new citizens appeared after adding food"
        );
    }

    #[test]
    fn death_moves_to_dead_columns() {
        let mut s = Sim::new(7);
        s.add_food(10_000);
        // Run long enough that at least one death fires (interval 100-300).
        for _ in 0..2000 {
            s.tick();
        }
        assert!(s.dead_civs() > 0, "no deaths fired in 2000 ticks");
        // No ID appears in both alive and dead.
        let alive: std::collections::HashSet<_> = s.civ_ids.iter().copied().collect();
        for &id in &s.dead_civ_ids {
            assert!(!alive.contains(&id), "id {} is both alive and dead", id);
        }
    }

    #[test]
    fn save_load_preserves_everything() {
        let mut a = Sim::new(7);
        a.add_food(10_000);
        a.set_cap(100);
        for _ in 0..2000 {
            a.tick();
        }
        assert!(a.dead_civs() > 0, "test needs at least one death");

        let path = std::env::temp_dir().join("city_sim_test_save");
        let _ = std::fs::remove_dir_all(&path);
        let path = path.to_str().unwrap();
        a.save(path).unwrap();

        let mut b = Sim::load(path).unwrap();
        assert_eq!(a.food, b.food);
        assert_eq!(a.population_cap, b.population_cap);
        assert_eq!(a.civ_ids, b.civ_ids);
        assert_eq!(a.civ_eq, b.civ_eq);
        assert_eq!(a.dead_civ_ids, b.dead_civ_ids);
        assert_eq!(a.dead_civ_eq, b.dead_civ_eq);
        assert_eq!(a.dead_civ_death_tick, b.dead_civ_death_tick);

        for _ in 0..200 {
            a.tick();
            b.tick();
        }
        assert_eq!(a.civ_eq, b.civ_eq);
        assert_eq!(a.food, b.food);
    }

    #[test]
    fn citizen_codes_are_unique() {
        let s = Sim::new(7);
        let mut seen = std::collections::HashSet::new();
        for i in 0..s.civs() as usize {
            assert!(seen.insert(s.civ_id_display(i)));
        }
    }
}
