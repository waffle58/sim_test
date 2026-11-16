use crate::random::stable_random;
use crate::names;
use serde::{Deserialize, Serialize};
use crate::tunables::Tunables;
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
const EVENT_COUPLE_SHUFFLE: u64 = 13;
const EVENT_COUPLE_INTERVAL: u64 = 14;
pub const MAX_CIV_ID: u32 = 999_999_999;

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
    pub civ_surname_from: Vec<u32>, 
    pub civ_base_eq: Vec<f32>,
    pub civ_household: Vec<u32>,
    pub household_ids: Vec<u32>,
    pub household_member_starts: Vec<u32>,
    pub household_member_ids: Vec<u32>,
    pub next_household_id: u32,
    pub(crate) next_couple_tick: u64,
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
        civ_surname_from: Vec::new(),
        civ_base_eq: Vec::new(),
        next_civ_id: 0,
        next_growth_tick: 0,
        next_death_tick: 0,
        dead_civ_ids: Vec::new(),
        dead_civ_eq: Vec::new(),
        dead_civ_death_tick: Vec::new(),
        total_deaths: 0,
        civ_household: Vec::new(),
        household_ids: Vec::new(),
        household_member_starts: vec![0],   // sentinel; empty table has starts=[0]
        household_member_ids: Vec::new(),
        next_household_id: 0,
        next_couple_tick: 0,
        tunables,
    };
    for _ in 0..sim.tunables.starting_citizens {
        if sim.spawn_civ(false).is_none() {
            break;
        }
    }
    sim.next_growth_tick = sim.tick_count + sim.roll_growth_interval(sim.tick_count);
    sim.next_death_tick = sim.tick_count + sim.roll_death_interval(sim.tick_count);
    sim.next_couple_tick = sim.tick_count + sim.roll_couple_interval(sim.tick_count);
    sim.bulk_form_couples(sim.tunables.initial_couple_percent);
    sim
    }

    pub fn spawn_civ(&mut self, is_native: bool) -> Option<u32> {
    if self.next_civ_id > MAX_CIV_ID {
        return None;
    }
    let id = self.next_civ_id;
    self.next_civ_id += 1;

    // EQ floor 0.10; mode 0.35
    let r_eq = stable_random(id as u64, EVENT_INITIAL_EQ, self.world_seed);
    let raw_eq = (r_eq % 100) as f32 / 100.0;
    let eq = 0.10 + triangular(raw_eq, 0.35) * 0.90;

    // Aptitude mode 0.5
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
    self.civ_surname_from.push(id);
    let h_id = self.create_household(id);
    self.civ_household.push(h_id);
    Some(id)
    }
    
    fn kill_civ(&mut self, idx: usize) {
        let civ_id = self.civ_ids[idx];
        let household_id = self.civ_household[idx];
        self.remove_member(household_id, civ_id);
        self.cull_household_if_empty(household_id);
        let id = self.civ_ids.swap_remove(idx);
        let eq = self.civ_eq.swap_remove(idx);
        self.civ_native_born.swap_remove(idx);
        self.civ_aptitude.swap_remove(idx);
        self.civ_female.swap_remove(idx);
        self.dead_civ_ids.push(id);
        self.dead_civ_eq.push(eq);
        self.dead_civ_death_tick.push(self.tick_count);
        self.civ_base_eq.swap_remove(idx);
        self.civ_surname_from.swap_remove(idx);
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
    fn create_household(&mut self, civ_id: u32) -> u32 {
    let h_id = self.next_household_id;
    self.next_household_id += 1;
    self.household_ids.push(h_id);
    self.household_member_ids.push(civ_id);
    self.household_member_starts.push(self.household_member_ids.len() as u32);
    h_id
    }
    fn add_member(&mut self, household_id: u32, civ_id: u32) {
        let Some(h_idx) = self.household_ids.iter().position(|&h| h == household_id) else { return; };
        let insert_pos = self.household_member_starts[h_idx + 1] as usize;
        self.household_member_ids.insert(insert_pos, civ_id);
        for i in (h_idx + 1)..self.household_member_starts.len() {
            self.household_member_starts[i] += 1;
        }
    }
    fn remove_member(&mut self, household_id: u32, civ_id: u32) {
        let Some(h_idx) = self.household_ids.iter().position(|&h| h == household_id) else { return; };
        let start = self.household_member_starts[h_idx] as usize;
        let end = self.household_member_starts[h_idx + 1] as usize;
        let Some(offset) = self.household_member_ids[start..end].iter().position(|&x| x == civ_id) else { return; };
        self.household_member_ids.remove(start + offset);
        for i in (h_idx + 1)..self.household_member_starts.len() {
            self.household_member_starts[i] -= 1;
        }
    }
    fn cull_household_if_empty(&mut self, household_id: u32) {
        let Some(h_idx) = self.household_ids.iter().position(|&h| h == household_id) else { return; };
        if self.household_member_starts[h_idx + 1] > self.household_member_starts[h_idx] {
            return;  // has members, keep
        }
        self.household_ids.remove(h_idx);
        self.household_member_starts.remove(h_idx);
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

        for i in 0..self.civ_ids.len() {
            let key = (self.civ_ids[i] as u64) ^ (self.tick_count << 32);

            if stable_random(key, EVENT_EQ_TRIGGER, self.world_seed)
                % self.tunables.eq_drift_period.max(1)
                != 0
            {
                continue;
            }

            let roll = stable_random(key, EVENT_EQ_DRIFT, self.world_seed) % 3;
            let delta = match roll {
                0 => -EQ_STEP,
                1 => 0.0,
                _ => EQ_STEP,
            };

            // Pull toward baseline
            let pull = (self.civ_base_eq[i] - self.civ_eq[i]) * 0.05;
            self.civ_eq[i] = quantize_eq((self.civ_eq[i] + delta + pull).clamp(0.0, 1.0));
        }
        if self.tick_count >= self.next_couple_tick {
        self.next_couple_tick = self.tick_count + self.roll_couple_interval(self.tick_count);
        self.bulk_form_couples(self.tunables.couple_rate_percent);
        }
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
            if self.spawn_civ(true).is_none() {
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
    pub fn female_count(&self) -> u32 {
    self.civ_female.iter().filter(|&&f| f).count() as u32
    }

    pub fn male_count(&self) -> u32 {
        self.civ_female.len() as u32 - self.female_count()
    }
    pub fn households(&self) -> u32 { self.household_ids.len() as u32 }
    pub fn household_member_count(&self, household_id: u32) -> u32 {
        let Some(h_idx) = self.household_ids.iter().position(|&h| h == household_id) else { return 0; };
        self.household_member_starts[h_idx + 1] - self.household_member_starts[h_idx]
    }
    pub fn household_eq_mean(&self, household_id: u32) -> f32 {
        let Some(h_idx) = self.household_ids.iter().position(|&h| h == household_id) else { return 0.0; };
        let start = self.household_member_starts[h_idx] as usize;
        let end = self.household_member_starts[h_idx + 1] as usize;
        if end <= start { return 0.0; }
        let sum: f32 = self.household_member_ids[start..end].iter()
            .map(|&civ_id| self.civ_ids.iter().position(|&c| c == civ_id)
                .map(|i| self.civ_eq[i]).unwrap_or(0.0))
            .sum();
        sum / (end - start) as f32
    }
    pub fn civ_household_of(&self, civ_id: u32) -> Option<u32> {
        self.civ_ids.iter().position(|&c| c == civ_id).map(|i| self.civ_household[i])
    }
    pub fn household_members(&self, household_id: u32) -> Vec<u32> {
        let Some(h_idx) = self.household_ids.iter().position(|&h| h == household_id) else { return Vec::new(); };
        let start = self.household_member_starts[h_idx] as usize;
        let end = self.household_member_starts[h_idx + 1] as usize;
        self.household_member_ids[start..end].to_vec()
    }
    fn roll_couple_interval(&self, at_tick: u64) -> u64 {
    let span = self.tunables.max_couple_interval
        .saturating_sub(self.tunables.min_couple_interval)
        .saturating_add(1)
        .max(1);
    let r = stable_random(at_tick, EVENT_COUPLE_INTERVAL, self.world_seed);
    self.tunables.min_couple_interval + r % span
    }
    fn form_couple(&mut self, male_id: u32, female_id: u32) {
        let Some(m_idx) = self.civ_ids.iter().position(|&c| c == male_id) else { return; };
        let Some(f_idx) = self.civ_ids.iter().position(|&c| c == female_id) else { return; };
        let male_h = self.civ_household[m_idx];
        let female_h = self.civ_household[f_idx];
        if male_h == female_h { return; }  // already together
        self.remove_member(female_h, female_id);
        self.add_member(male_h, female_id);
        self.civ_household[f_idx] = male_h;
        self.cull_household_if_empty(female_h);
        let head_surname = self.civ_surname_from[m_idx];
    self.civ_surname_from[f_idx] = head_surname;
    }
    fn bulk_form_couples(&mut self, percent: u32) {
        if self.civ_ids.len() < 2 || percent == 0 { return; }
        let mut single_males: Vec<u32> = Vec::new();
        let mut single_females: Vec<u32> = Vec::new();
        for i in 0..self.civ_ids.len() {
            let h = self.civ_household[i];
            if self.household_member_count(h) != 1 { continue; }
            if self.civ_female[i] { single_females.push(self.civ_ids[i]); }
            else { single_males.push(self.civ_ids[i]); }
        }
        let possible = single_males.len().min(single_females.len());
        if possible == 0 { return; }
        let target = ((possible as f32) * percent as f32 / 100.0).ceil() as usize;
        if target == 0 { return; }
        single_males.sort_by_key(|&id| stable_random(id as u64, EVENT_COUPLE_SHUFFLE, self.world_seed));
        single_females.sort_by_key(|&id| stable_random(id as u64, EVENT_COUPLE_SHUFFLE, self.world_seed));
        for i in 0..target {
            self.form_couple(single_males[i], single_females[i]);
        }
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
            surname: derive_surname(self.civ_surname_from[i], self.world_seed),
            birth_surname: derive_surname(id, self.world_seed),
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
            birth_surname: derive_surname(id, self.world_seed),
        });
    }
    None
    }
    pub fn population_stats(&self) -> PopStats {
        let n = self.civ_ids.len();
        if n == 0 {
            return PopStats {
                total: 0, female: 0, male: 0,
                eq_mean: 0.0, eq_min: 0.0, eq_max: 0.0, eq_mode: 0.0,
                aptitude_mean: 0.0, aptitude_min: 0.0, aptitude_max: 0.0, aptitude_mode: 0.0,
            };
        }

        let mut female: u32 = 0;
        let mut eq_sum: f64 = 0.0;
        let mut eq_min = f32::INFINITY;
        let mut eq_max = f32::NEG_INFINITY;
        let mut eq_buckets = [0u32; 101];

        let mut apt_sum: f64 = 0.0;
        let mut apt_min = f32::INFINITY;
        let mut apt_max = f32::NEG_INFINITY;
        let mut apt_buckets = [0u32; 101];

        for i in 0..n {
            if self.civ_female[i] { female += 1; }

            let eq = self.civ_eq[i];
            eq_sum += eq as f64;
            if eq < eq_min { eq_min = eq; }
            if eq > eq_max { eq_max = eq; }
            eq_buckets[((eq * 100.0).round() as usize).min(100)] += 1;

            let apt = self.civ_aptitude[i];
            apt_sum += apt as f64;
            if apt < apt_min { apt_min = apt; }
            if apt > apt_max { apt_max = apt; }
            apt_buckets[((apt * 100.0).round() as usize).min(100)] += 1;
        }

        let eq_mode_bucket = eq_buckets.iter().enumerate()
            .max_by_key(|(_, &c)| c).map(|(i, _)| i).unwrap_or(0);
        let apt_mode_bucket = apt_buckets.iter().enumerate()
            .max_by_key(|(_, &c)| c).map(|(i, _)| i).unwrap_or(0);

        PopStats {
            total: n as u32,
            female,
            male: n as u32 - female,
            eq_mean: (eq_sum / n as f64) as f32,
            eq_min,
            eq_max,
            eq_mode: eq_mode_bucket as f32 / 100.0,
            aptitude_mean: (apt_sum / n as f64) as f32,
            aptitude_min: apt_min,
            aptitude_max: apt_max,
            aptitude_mode: apt_mode_bucket as f32 / 100.0,
        }
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
    pub birth_surname: &'static str,
}
pub struct PopStats {
    pub total: u32,
    pub female: u32,
    pub male: u32,
    pub eq_mean: f32,
    pub eq_min: f32,
    pub eq_max: f32,
    pub eq_mode: f32,
    pub aptitude_mean: f32,
    pub aptitude_min: f32,
    pub aptitude_max: f32,
    pub aptitude_mode: f32,
}
