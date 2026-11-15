//! The simulation state and its tick loop.

use crate::random::stable_random;
use crate::names;
use serde::{Deserialize, Serialize};
use crate::tunables::Tunables;
// --- Non-tunable constants ---
const EQ_STEP: f32 = 0.02;
const EVENT_INITIAL_EQ: u64 = 3;
const EVENT_EQ_TRIGGER: u64 = 4;
const EVENT_EQ_DRIFT: u64 = 5;
const EVENT_POP_GROWTH: u64 = 1;
const EVENT_POP_INTERVAL: u64 = 2;
const EVENT_DEATH_INTERVAL: u64 = 6;
const EVENT_DEATH_COUNT: u64 = 7;
const EVENT_DEATH_PICK: u64 = 8;
const EVENT_INITIAL_APTITUDE: u64 = 9;
const EVENT_GIVEN_NAME: u64 = 10;
const EVENT_SURNAME: u64 = 11;
const EVENT_GENDER: u64 = 12;
pub const MAX_CIV_ID: u32 = 999_999_999;

// --- Sim ---
#[derive(Serialize, Deserialize)]
pub struct Sim {
    pub world_seed: u64,
    pub tick_count: u64,

    pub food: i64,
    pub population_cap: u32,
    pub tunables: Tunables,

    pub civ_ids: Vec<u32>,
    pub civ_eq: Vec<f32>,
    pub next_civ_id: u32,
    pub civ_native_born: Vec<bool>,
    pub civ_aptitude: Vec<f32>,
    pub civ_female: Vec<bool>,
    pub civ_base_eq: Vec<f32>,
    pub(crate) next_growth_tick: u64,
    pub(crate) next_death_tick: u64,

    pub dead_civ_ids: Vec<u32>,
    pub dead_civ_eq: Vec<f32>,
    pub dead_civ_death_tick: Vec<u64>,
    pub total_deaths: u64,
}

fn quantize_eq(x: f32) -> f32 {
    (x * 100.0).round() / 100.0
}
fn triangular(u: f32, mode: f32) -> f32 {
    if u < mode { (u * mode).sqrt() }
    else { 1.0 - ((1.0 - u) * (1.0 - mode)).sqrt() }
}

fn derive_gender(id: u32, seed: u64) -> bool {
    ((stable_random(id as u64, EVENT_GENDER, seed) >> 32) & 1) == 0
}

fn derive_given(id: u32, female: bool, seed: u64) -> &'static str {
    let list = if female { names::GIVEN_FEMALE } else { names::GIVEN_MALE };
    list[(stable_random(id as u64, EVENT_GIVEN_NAME, seed) as usize) % list.len()]
}

fn derive_surname(id: u32, seed: u64) -> &'static str {
    names::SURNAMES[(stable_random(id as u64, EVENT_SURNAME, seed) as usize) % names::SURNAMES.len()]
}

impl Sim {
    pub fn new(world_seed: u64) -> Self {
        Self::new_with_tunables(world_seed, Tunables::default())
    }

    pub fn new_with_tunables(world_seed: u64, tunables: Tunables) -> Self {
    let mut sim = Sim {
        world_seed,
        tick_count: 0,
        food: tunables.starting_food,
        population_cap: tunables.default_cap,
        civ_ids: Vec::new(),
        civ_eq: Vec::new(),
        civ_native_born: Vec::new(),
        civ_aptitude: Vec::new(),
        civ_female: Vec::new(),
        civ_base_eq: Vec::new(),
        next_civ_id: 0,
        next_growth_tick: 0,
        next_death_tick: 0,
        dead_civ_ids: Vec::new(),
        dead_civ_eq: Vec::new(),
        dead_civ_death_tick: Vec::new(),
        total_deaths: 0,
        tunables,
    };
    for _ in 0..sim.tunables.starting_citizens {
        if sim.spawn_civ(false).is_none() {
            break;
        }
    }
    sim.next_growth_tick = sim.tick_count + sim.roll_growth_interval(sim.tick_count);
    sim.next_death_tick = sim.tick_count + sim.roll_death_interval(sim.tick_count);
    sim
    }

    pub fn spawn_civ(&mut self, is_native: bool) -> Option<u32> {
    if self.next_civ_id > MAX_CIV_ID {
        return None;
    }
    let id = self.next_civ_id;
    self.next_civ_id += 1;

    // Initial eq: floor at 0.10, triangular centered at 0.35
    let r_eq = stable_random(id as u64, EVENT_INITIAL_EQ, self.world_seed);
    let raw_eq = (r_eq % 100) as f32 / 100.0;
    let eq = 0.10 + triangular(raw_eq, 0.35) * 0.90;

    // Aptitude: symmetric triangular, centered at 0.5
    let r_apt = stable_random(id as u64, EVENT_INITIAL_APTITUDE, self.world_seed);
    let raw_apt = (r_apt % 100) as f32 / 100.0;
    let aptitude = quantize_eq(triangular(raw_apt, 0.5));

    let eq_final = quantize_eq(eq);
    self.civ_eq.push(eq_final);
    self.civ_base_eq.push(eq_final);

    let female = derive_gender(id, self.world_seed);

    self.civ_ids.push(id);
    self.civ_eq.push(quantize_eq(eq));
    self.civ_native_born.push(is_native);
    self.civ_aptitude.push(aptitude);
    self.civ_female.push(female);
    Some(id)
    }
    
    fn kill_civ(&mut self, idx: usize) {
        let id = self.civ_ids.swap_remove(idx);
        let eq = self.civ_eq.swap_remove(idx);
        self.civ_native_born.swap_remove(idx);
        self.civ_aptitude.swap_remove(idx);
        self.civ_female.swap_remove(idx);
        self.dead_civ_ids.push(id);
        self.dead_civ_eq.push(eq);
        self.dead_civ_death_tick.push(self.tick_count);
        self.civ_base_eq.swap_remove(idx);
        self.total_deaths += 1;
        self.cull_dead();
    }

    fn cull_dead(&mut self) {
        let cap = self.tunables.max_dead_retained as usize;
        if cap == 0 || self.dead_civ_ids.len() <= cap {
            return;
        }
        let trim = (cap / 10).max(1);
        self.dead_civ_ids.drain(0..trim);
        self.dead_civ_eq.drain(0..trim);
        self.dead_civ_death_tick.drain(0..trim);
    }
    fn roll_death_victim(&self, kill_idx: u32) -> Option<usize> {
        let n = self.civ_ids.len();
        if n == 0 {
            return None;
        }
        let key = (kill_idx as u64) ^ (self.tick_count << 8);
        let r = stable_random(key, EVENT_DEATH_PICK, self.world_seed);
        Some((r as usize) % n)
    }

    fn roll_growth_interval(&self, at_tick: u64) -> u64 {
        let r = stable_random(at_tick, EVENT_POP_INTERVAL, self.world_seed);
        let span = self
            .tunables
            .max_growth_interval
            .saturating_sub(self.tunables.min_growth_interval)
            .saturating_add(1)
            .max(1);
        self.tunables.min_growth_interval + r % span
    }

    fn roll_death_interval(&self, at_tick: u64) -> u64 {
        let r = stable_random(at_tick, EVENT_DEATH_INTERVAL, self.world_seed);
        let span = self
            .tunables
            .max_death_interval
            .saturating_sub(self.tunables.min_death_interval)
            .saturating_add(1)
            .max(1);
        self.tunables.min_death_interval + r % span
    }

    pub fn tick(&mut self) -> (u32, u32) {
        self.tick_count += 1;
        let mut died = 0u32;

        // 1. Per-citizen eq drift
        for i in 0..self.civ_ids.len() {
            let key = (self.civ_ids[i] as u64) ^ (self.tick_count << 32);

            // Trigger: does this citizen drift this tick?
            if stable_random(key, EVENT_EQ_TRIGGER, self.world_seed)
                % self.tunables.eq_drift_period.max(1)
                != 0
            {
                continue;
            }

            // Direction: -step, 0, or +step
            let roll = stable_random(key, EVENT_EQ_DRIFT, self.world_seed) % 3;
            let delta = match roll {
                0 => -EQ_STEP,
                1 => 0.0,
                _ => EQ_STEP,
            };

            // Mean reversion: pull toward this citizen's baseline.
            let pull = (self.civ_base_eq[i] - self.civ_eq[i]) * 0.05;
            self.civ_eq[i] = quantize_eq((self.civ_eq[i] + delta + pull).clamp(0.0, 1.0));
        }
        // 2. Death event
        if self.tick_count >= self.next_death_tick {
            self.next_death_tick = self.tick_count + self.roll_death_interval(self.tick_count);
            let r = stable_random(self.tick_count, EVENT_DEATH_COUNT, self.world_seed);
            let kills = (r % (self.tunables.max_death as u64 + 1)) as u32;
            for kill_idx in 0..kills {
                if let Some(victim) = self.roll_death_victim(kill_idx) {
                    self.kill_civ(victim);
                    died += 1;
                }
            }
        }

        // 3. Growth event
        if self.tick_count < self.next_growth_tick {
            return (0, died);
        }
        self.next_growth_tick = self.tick_count + self.roll_growth_interval(self.tick_count);

        let alive = self.civ_ids.len() as u32;
        if alive >= self.population_cap {
            return (0, died);
        }

        let r = stable_random(self.tick_count, EVENT_POP_GROWTH, self.world_seed);
        let wanted = (r % (self.tunables.max_growth as u64 + 1)) as u32;
        let room = self.population_cap - alive;
        let affordable = if self.food <= 0 || self.tunables.food_per_spawn <= 0 {
            0
        } else {
            (self.food / self.tunables.food_per_spawn) as u32
        };
        let actual = wanted.min(room).min(affordable);

        let mut spawned = 0;
        for _ in 0..actual {
            if self.spawn_civ(true).is_none() {   // true = birth (native born)
                break;
            }
            spawned += 1;
        }
        self.food -= (spawned as i64) * self.tunables.food_per_spawn;
        (spawned, died)
    }

    pub fn civs(&self) -> u32 {
        self.civ_ids.len() as u32
    }
    pub fn dead_civs(&self) -> u32 {
        self.dead_civ_ids.len() as u32
    }
    pub fn total_deaths(&self) -> u64 {
        self.total_deaths
    }
    pub fn add_food(&mut self, amount: i64) {
        self.food += amount;
    }
    pub fn set_cap(&mut self, cap: u32) {
        self.population_cap = cap;
    }

    pub fn civ(&self, i: usize) -> Civ {
        Civ {
            id: self.civ_ids[i],
            eq: self.civ_eq[i],
        }
    }

    pub fn dead_civ(&self, i: usize) -> DeadCiv {
        DeadCiv {
            id: self.dead_civ_ids[i],
            eq: self.dead_civ_eq[i],
            death_tick: self.dead_civ_death_tick[i],
        }
    }

    pub fn civ_id_display(&self, i: usize) -> String {
        format!("{:09}", self.civ_ids[i])
    }

    pub fn dead_civ_id_display(&self, i: usize) -> String {
        format!("{:09}", self.dead_civ_ids[i])
    }
    pub fn find_by_id(&self, id: u32) -> Option<InspectView> {
    if let Some(i) = self.civ_ids.iter().position(|&c| c == id) {
        let female = self.civ_female[i];
        return Some(InspectView {
            id,
            alive: true,
            death_tick: None,
            female,
            native_born: self.civ_native_born[i],
            eq: self.civ_eq[i],
            aptitude: self.civ_aptitude[i],
            given_name: derive_given(id, female, self.world_seed),
            surname: derive_surname(id, self.world_seed),
        });
    }
    if let Some(i) = self.dead_civ_ids.iter().position(|&c| c == id) {
        let female = derive_gender(id, self.world_seed);
        return Some(InspectView {
            id,
            alive: false,
            death_tick: Some(self.dead_civ_death_tick[i]),
            female,
            native_born: false,
            eq: self.dead_civ_eq[i],
            aptitude: f32::NAN,
            given_name: derive_given(id, female, self.world_seed),
            surname: derive_surname(id, self.world_seed),
        });
    }
    None
}
}
#[derive(Clone, Copy, Debug)]
pub struct Civ {
    pub id: u32,
    pub eq: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DeadCiv {
    pub id: u32,
    pub eq: f32,
    pub death_tick: u64,
}
pub struct InspectView {
    pub id: u32,
    pub alive: bool,
    pub death_tick: Option<u64>,
    pub female: bool,
    pub native_born: bool,
    pub eq: f32,
    pub aptitude: f32,
    pub given_name: &'static str,
    pub surname: &'static str,
}