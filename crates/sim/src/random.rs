fn hash64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

pub fn stable_random(entity_id: u64, event: u64, world_seed: u64) -> u64 {
    hash64(entity_id ^ hash64(event ^ world_seed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_random_is_stable() {
        assert_eq!(stable_random(1, 0, 9), stable_random(1, 0, 9));
        assert_ne!(stable_random(1, 0, 9), stable_random(2, 0, 9));
        assert_ne!(stable_random(1, 0, 9), stable_random(1, 1, 9));
        assert_ne!(stable_random(1, 0, 9), stable_random(1, 0, 10));
    }

    #[test]
    fn produces_varied_values() {
        let mut distinct = std::collections::HashSet::new();
        for i in 0..1000 {
            distinct.insert(stable_random(i, 0, 42));
        }
        assert_eq!(distinct.len(), 1000, "values collided unexpectedly");
    }
}