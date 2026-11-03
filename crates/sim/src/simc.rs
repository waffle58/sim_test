use serde::{Deserialize, Serialize};
use crate::random::stable_random;

// pop growth
const STARTING_POPULATION: u32 = 100;
pub const POPULATION_CAP: u32 = 500;
const MIN_GROWTH_INTERVAL: u64 = 1;  // ticks between growth events
const MAX_GROWTH_INTERVAL: u64 = 5;
const MAX_GROWTH: u64 = 3;           // max added per event

// event ids
const EVENT_POP_GROWTH: u64 = 1;
const EVENT_POP_INTERVAL: u64 = 2;

#[derive(Serialize, Deserialize)]
pub struct Sim {
    pub world_seed: u64,
    pub tick_count: u64,

    pub population: u32,

    next_growth_tick: u64,
}

impl Sim {
    pub fn new(world_seed: u64) -> Self {
        let mut sim = Sim {
            world_seed,
            tick_count: 0,
            population: STARTING_POPULATION,
            next_growth_tick: 0,
        };
        sim.next_growth_tick = sim.roll_interval(0);
        sim
    }

    fn roll_interval(&self, at_tick: u64) -> u64 {
        let r = stable_random(at_tick, EVENT_POP_INTERVAL, self.world_seed);
        let span = MAX_GROWTH_INTERVAL - MIN_GROWTH_INTERVAL + 1;
        MIN_GROWTH_INTERVAL + r % span
    }


    pub fn tick(&mut self) -> u32 {
        self.tick_count += 1;

        if self.tick_count < self.next_growth_tick {
            return 0;
        }

        self.next_growth_tick = self.tick_count + self.roll_interval(self.tick_count);

        if self.population >= POPULATION_CAP {
            return 0;
        }

        let r = stable_random(self.tick_count, EVENT_POP_GROWTH, self.world_seed);
        let gain = (r % (MAX_GROWTH + 1)) as u32;
        let room = POPULATION_CAP - self.population;
        let actual = gain.min(room);
        self.population += actual;
        actual
    }
        pub fn save(&self, path: &str) -> std::io::Result<()> {
        let bytes = bincode::serialize(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, bytes)
    }

    pub fn load(path: &str) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        bincode::deserialize(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
    pub fn citizens(&self) -> u32 {
        self.population
    }
}