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
    assert_eq!(a.citizen_eq, b.citizen_eq);
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
    let eqs: Vec<f32> =
        (0..s.citizens() as usize).map(|i| s.citizen(i).eq).collect();
    let min = eqs.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = eqs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!(max - min > 0.5, "eqs clustered: {} .. {}", min, max);
}

#[test]
fn save_load_preserves_citizens() {
    let mut a = Sim::new(7);
    for _ in 0..200 { a.tick(); }
    let path = std::env::temp_dir().join("city_sim_test_save");
    let _ = std::fs::remove_dir_all(&path);
    let path = path.to_str().unwrap();
    a.save(path).unwrap();

    let mut b = Sim::load(path).unwrap();
    assert_eq!(a.citizen_ids, b.citizen_ids);
    assert_eq!(a.citizen_eq, b.citizen_eq);
    assert_eq!(a.next_citizen_id, b.next_citizen_id);

    for _ in 0..100 { a.tick(); b.tick(); }
    assert_eq!(a.citizen_eq, b.citizen_eq);
}
#[test]
fn citizen_codes_are_unique() {
    let mut s = Sim::new(7);
    for _ in 0..10_000 { s.tick(); }  // fills to cap 50

    let mut seen = std::collections::HashSet::new();
    for i in 0..s.citizens() as usize {
        let code = s.citizen_code(i);
        assert!(seen.insert(code.clone()), "duplicate code: {}", code);
    }
}

#[test]
fn citizen_codes_are_10_crockford_chars() {
    let s = Sim::new(7);
    for i in 0..s.citizens() as usize {
        let code = s.citizen_code(i);
        assert_eq!(code.len(), 10);
        for ch in code.chars() {
            assert!(
                "0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(ch),
                "invalid char '{}' in code {}",
                ch, code
            );
        }
    }
}

#[test]
fn eq_is_two_decimals() {
    let mut s = Sim::new(7);
    for _ in 0..2_000 { s.tick(); }
    for i in 0..s.citizens() as usize {
        let eq = s.citizen(i).eq;
        let rounded = (eq * 100.0).round() / 100.0;
        assert!((eq - rounded).abs() < 1e-5, "eq {} not on 2-decimal grid", eq);
    }
}
}