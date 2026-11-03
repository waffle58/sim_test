//! The simulation state and its tick loop.

use crate::random::stable_random;
use serde::{Deserialize, Serialize};

// --- Population growth ---
const STARTING_CITIZENS: u32 = 10;
pub const POPULATION_CAP: u32 = 50;
const MIN_GROWTH_INTERVAL: u64 = 1;
const MAX_GROWTH_INTERVAL: u64 = 5;
const MAX_GROWTH: u32 = 3;
const EVENT_POP_GROWTH: u64 = 1;
const EVENT_POP_INTERVAL: u64 = 2;

// --- Per-citizen decisions ---
const MIN_DECISION_INTERVAL: u64 = 5;
const MAX_DECISION_INTERVAL: u64 = 30;
const CAPABILITY_STEP: f32 = 0.02;
const EVENT_INITIAL_CAPABILITY: u64 = 3;
const EVENT_DECISION_INTERVAL: u64 = 4;
const EVENT_CAPABILITY_DRIFT: u64 = 5;

#[derive(Serialize, Deserialize)]
pub struct Sim {
    pub world_seed: u64,
    pub tick_count: u64,

    // Citizens (SoA). Index i in each column is the same citizen.
    pub citizen_ids: Vec<u32>,
    pub citizen_capability: Vec<f32>,
    pub citizen_next_decision: Vec<u64>,
    pub next_citizen_id: u32,

    next_growth_tick: u64,
}

impl Sim {
    pub fn new(world_seed: u64) -> Self {
        let mut sim = Sim {
            world_seed,
            tick_count: 0,
            citizen_ids: Vec::new(),
            citizen_capability: Vec::new(),
            citizen_next_decision: Vec::new(),
            next_citizen_id: 0,
            next_growth_tick: 0,
        };
        for _ in 0..STARTING_CITIZENS {
            sim.spawn_citizen();
        }
        sim.next_growth_tick = sim.tick_count + sim.roll_growth_interval(sim.tick_count);
        sim
    }

    /// Create one citizen with an id stable across runs.
    pub fn spawn_citizen(&mut self) -> u32 {
        let id = self.next_citizen_id;
        self.next_citizen_id += 1;

        let r = stable_random(id as u64, EVENT_INITIAL_CAPABILITY, self.world_seed);
        let capability = (r % 10_000) as f32 / 10_000.0;
        let next = self.tick_count + self.roll_decision_interval(id, self.tick_count);

        self.citizen_ids.push(id);
        self.citizen_capability.push(capability);
        self.citizen_next_decision.push(next);
        id
    }

    fn roll_growth_interval(&self, at_tick: u64) -> u64 {
        let r = stable_random(at_tick, EVENT_POP_INTERVAL, self.world_seed);
        let span = MAX_GROWTH_INTERVAL - MIN_GROWTH_INTERVAL + 1;
        MIN_GROWTH_INTERVAL + r % span
    }

    fn roll_decision_interval(&self, id: u32, at_tick: u64) -> u64 {
        let key = (id as u64) ^ (at_tick << 32);
        let r = stable_random(key, EVENT_DECISION_INTERVAL, self.world_seed);
        let span = MAX_DECISION_INTERVAL - MIN_DECISION_INTERVAL + 1;
        MIN_DECISION_INTERVAL + r % span
    }

    pub fn tick(&mut self) -> u32 {
        self.tick_count += 1;

        // 1. Per-citizen decisions
        for i in 0..self.citizen_ids.len() {
            if self.tick_count < self.citizen_next_decision[i] {
                continue;
            }
            let id = self.citizen_ids[i];
            let key = (id as u64) ^ (self.tick_count << 32);

            let roll = stable_random(key, EVENT_CAPABILITY_DRIFT, self.world_seed) % 3;
            let delta = match roll {
                0 => -CAPABILITY_STEP,
                1 => 0.0,
                _ => CAPABILITY_STEP,
            };
            self.citizen_capability[i] =
                (self.citizen_capability[i] + delta).clamp(0.0, 1.0);

            self.citizen_next_decision[i] =
                self.tick_count + self.roll_decision_interval(id, self.tick_count);
        }

        // 2. Population growth
        if self.tick_count < self.next_growth_tick {
            return 0;
        }
        self.next_growth_tick = self.tick_count + self.roll_growth_interval(self.tick_count);

        let count = self.citizen_ids.len() as u32;
        if count >= POPULATION_CAP {
            return 0;
        }

        let r = stable_random(self.tick_count, EVENT_POP_GROWTH, self.world_seed);
        let gain = (r % (MAX_GROWTH as u64 + 1)) as u32;
        let room = POPULATION_CAP - count;
        let actual = gain.min(room);
        for _ in 0..actual {
            self.spawn_citizen();
        }
        actual
    }

    pub fn citizens(&self) -> u32 {
        self.citizen_ids.len() as u32
    }

    /// Read-only view of one citizen. Used by the runner and later the bridge.
    pub fn citizen(&self, i: usize) -> CitizenView {
        CitizenView {
            id: self.citizen_ids[i],
            capability: self.citizen_capability[i],
            next_decision: self.citizen_next_decision[i],
        }
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
}

#[derive(Clone, Copy, Debug)]
pub struct CitizenView {
    pub id: u32,
    pub capability: f32,
    pub next_decision: u64,
}