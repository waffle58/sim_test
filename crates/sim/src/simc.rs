//! The simulation state and its tick loop.

use crate::random::stable_random;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

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
pub const MAX_CITIZEN_ID: u32 = 999_999_999;
const SAVE_VERSION: u32 = 4;
const CITIZENS_PER_SHARD: usize = 10_000;


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tunables {
    pub starting_citizens: u32,
    pub starting_food: i64,
    pub default_cap: u32,
    pub food_per_spawn: i64,
    pub min_growth_interval: u64,
    pub max_growth_interval: u64,
    pub max_growth: u32,
    pub min_death_interval: u64,
    pub max_death_interval: u64,
    pub max_death: u32,
    pub eq_drift_period: u64,
}

impl Default for Tunables {
    fn default() -> Self {
        Tunables {
            starting_citizens: 10,
            starting_food: 100,
            default_cap: 50,
            food_per_spawn: 10,
            min_growth_interval: 35,
            max_growth_interval: 45,
            max_growth: 3,
            min_death_interval: 95,
            max_death_interval: 105,
            max_death: 2,
            eq_drift_period: 17,
        }
    }
}

impl Tunables {
    pub fn from_file(path: &str) -> std::io::Result<Self> {
        let s = fs::read_to_string(path)?;
        serde_json::from_str(&s)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn to_file(&self, path: &str) -> std::io::Result<()> {
        let s = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(path, s)
    }

    /// Set one field by name from a string value. Returns Err on unknown field or bad parse.
    pub fn set_field(&mut self, field: &str, value: &str) -> Result<(), String> {
        let parse_u64 = |v: &str| v.parse::<u64>().map_err(|_| format!("invalid number '{}'", v));
        let parse_i64 = |v: &str| v.parse::<i64>().map_err(|_| format!("invalid number '{}'", v));
        let parse_u32 = |v: &str| v.parse::<u32>().map_err(|_| format!("invalid number '{}'", v));

        match field {
            "starting_citizens"    => self.starting_citizens = parse_u32(value)?,
            "starting_food"        => self.starting_food = parse_i64(value)?,
            "default_cap"          => self.default_cap = parse_u32(value)?,
            "food_per_spawn"       => self.food_per_spawn = parse_i64(value)?,
            "min_growth_interval"  => self.min_growth_interval = parse_u64(value)?,
            "max_growth_interval"  => self.max_growth_interval = parse_u64(value)?,
            "max_growth"           => self.max_growth = parse_u32(value)?,
            "min_death_interval"   => self.min_death_interval = parse_u64(value)?,
            "max_death_interval"   => self.max_death_interval = parse_u64(value)?,
            "max_death"            => self.max_death = parse_u32(value)?,
            "eq_drift_period"      => self.eq_drift_period = parse_u64(value)?,
            other => return Err(format!("unknown field '{}'", other)),
        }

        // Guard against accidental min > max (which would make span roll panic).
        if self.min_growth_interval > self.max_growth_interval {
            return Err(format!(
                "min_growth_interval ({}) > max_growth_interval ({})",
                self.min_growth_interval, self.max_growth_interval));
        }
        if self.min_death_interval > self.max_death_interval {
            return Err(format!(
                "min_death_interval ({}) > max_death_interval ({})",
                self.min_death_interval, self.max_death_interval));
        }
        Ok(())
    }

    pub fn to_lines(&self) -> Vec<String> {
        vec![
            format!("starting_citizens    = {}", self.starting_citizens),
            format!("starting_food        = {}", self.starting_food),
            format!("default_cap          = {}", self.default_cap),
            format!("food_per_spawn       = {}", self.food_per_spawn),
            format!("min_growth_interval  = {} ticks ({:.2}s)", self.min_growth_interval, self.min_growth_interval as f64 / 20.0),
            format!("max_growth_interval  = {} ticks ({:.2}s)", self.max_growth_interval, self.max_growth_interval as f64 / 20.0),
            format!("max_growth           = {}", self.max_growth),
            format!("min_death_interval   = {} ticks ({:.2}s)", self.min_death_interval, self.min_death_interval as f64 / 20.0),
            format!("max_death_interval   = {} ticks ({:.2}s)", self.max_death_interval, self.max_death_interval as f64 / 20.0),
            format!("max_death            = {}", self.max_death),
            format!("eq_drift_period      = {} ticks", self.eq_drift_period),
        ]
    }
}

// --- Sim ---

#[derive(Serialize, Deserialize)]
pub struct Sim {
    pub world_seed: u64,
    pub tick_count: u64,

    pub food: i64,
    pub population_cap: u32,
    pub tunables: Tunables,

    pub citizen_ids: Vec<u32>,
    pub citizen_eq: Vec<f32>,
    pub next_citizen_id: u32,
    next_growth_tick: u64,
    next_death_tick: u64,

    pub dead_citizen_ids: Vec<u32>,
    pub dead_citizen_eq: Vec<f32>,
    pub dead_citizen_death_tick: Vec<u64>,
}

#[derive(Serialize, Deserialize)]
struct Meta {
    version: u32,
    world_seed: u64,
    tick_count: u64,
    food: i64,
    population_cap: u32,
    tunables: Tunables,
    next_citizen_id: u32,
    next_growth_tick: u64,
    next_death_tick: u64,
}

#[derive(Serialize, Deserialize)]
struct Shard {
    ids: Vec<u32>,
    eq: Vec<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    death_tick: Vec<u64>,
}

struct ColumnarFormatter;
impl serde_json::ser::Formatter for ColumnarFormatter {
    fn begin_array<W>(&mut self, w: &mut W) -> std::io::Result<()> where W: ?Sized + std::io::Write { w.write_all(b"[") }
    fn end_array<W>(&mut self, w: &mut W) -> std::io::Result<()> where W: ?Sized + std::io::Write { w.write_all(b"]") }
    fn begin_array_value<W>(&mut self, w: &mut W, first: bool) -> std::io::Result<()> where W: ?Sized + std::io::Write {
        if !first { w.write_all(b",")?; } Ok(())
    }
    fn begin_object<W>(&mut self, w: &mut W) -> std::io::Result<()> where W: ?Sized + std::io::Write { w.write_all(b"{") }
    fn end_object<W>(&mut self, w: &mut W) -> std::io::Result<()> where W: ?Sized + std::io::Write { w.write_all(b"\n}") }
    fn begin_object_key<W>(&mut self, w: &mut W, first: bool) -> std::io::Result<()> where W: ?Sized + std::io::Write {
        if first { w.write_all(b"\n  ")?; } else { w.write_all(b",\n  ")?; } Ok(())
    }
    fn begin_object_value<W>(&mut self, w: &mut W) -> std::io::Result<()> where W: ?Sized + std::io::Write {
        w.write_all(b": ")
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, ColumnarFormatter);
    value.serialize(&mut ser).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(path, buf)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> std::io::Result<T> {
    let s = fs::read_to_string(path)?;
    serde_json::from_str(&s).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn quantize_eq(x: f32) -> f32 {
    (x * 100.0).round() / 100.0
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
            citizen_ids: Vec::new(),
            citizen_eq: Vec::new(),
            next_citizen_id: 0,
            next_growth_tick: 0,
            next_death_tick: 0,
            dead_citizen_ids: Vec::new(),
            dead_citizen_eq: Vec::new(),
            dead_citizen_death_tick: Vec::new(),
            tunables,
        };
        for _ in 0..sim.tunables.starting_citizens {
            if sim.spawn_citizen().is_none() { break; }
        }
        sim.next_growth_tick = sim.tick_count + sim.roll_growth_interval(sim.tick_count);
        sim.next_death_tick = sim.tick_count + sim.roll_death_interval(sim.tick_count);
        sim
    }

    pub fn spawn_citizen(&mut self) -> Option<u32> {
        if self.next_citizen_id > MAX_CITIZEN_ID {
            return None;
        }
        let id = self.next_citizen_id;
        self.next_citizen_id += 1;
        let r = stable_random(id as u64, EVENT_INITIAL_EQ, self.world_seed);
        let eq = (r % 100) as f32 / 100.0;
        self.citizen_ids.push(id);
        self.citizen_eq.push(eq);
        Some(id)
    }

    fn kill_citizen(&mut self, idx: usize) {
        let id = self.citizen_ids.swap_remove(idx);
        let eq = self.citizen_eq.swap_remove(idx);
        self.dead_citizen_ids.push(id);
        self.dead_citizen_eq.push(eq);
        self.dead_citizen_death_tick.push(self.tick_count);
    }

    fn roll_death_victim(&self, kill_idx: u32) -> Option<usize> {
        let n = self.citizen_ids.len();
        if n == 0 { return None; }
        let key = (kill_idx as u64) ^ (self.tick_count << 8);
        let r = stable_random(key, EVENT_DEATH_PICK, self.world_seed);
        Some((r as usize) % n)
    }

    fn roll_growth_interval(&self, at_tick: u64) -> u64 {
        let r = stable_random(at_tick, EVENT_POP_INTERVAL, self.world_seed);
        let span = self.tunables.max_growth_interval
            .saturating_sub(self.tunables.min_growth_interval)
            .saturating_add(1)
            .max(1);
        self.tunables.min_growth_interval + r % span
    }

    fn roll_death_interval(&self, at_tick: u64) -> u64 {
        let r = stable_random(at_tick, EVENT_DEATH_INTERVAL, self.world_seed);
        let span = self.tunables.max_death_interval
            .saturating_sub(self.tunables.min_death_interval)
            .saturating_add(1)
            .max(1);
        self.tunables.min_death_interval + r % span
    }

pub fn tick(&mut self) -> (u32, u32) {
    self.tick_count += 1;
    let mut died = 0u32;

    // 1. Per-citizen eq drift
    for i in 0..self.citizen_ids.len() {
        let key = (self.citizen_ids[i] as u64) ^ (self.tick_count << 32);
        if stable_random(key, EVENT_EQ_TRIGGER, self.world_seed) % self.tunables.eq_drift_period.max(1) != 0 {
            continue;
        }
        let roll = stable_random(key, EVENT_EQ_DRIFT, self.world_seed) % 3;
        let delta = match roll { 0 => -EQ_STEP, 1 => 0.0, _ => EQ_STEP };
        self.citizen_eq[i] = quantize_eq((self.citizen_eq[i] + delta).clamp(0.0, 1.0));
    }

    // 2. Death event
    if self.tick_count >= self.next_death_tick {
        self.next_death_tick = self.tick_count + self.roll_death_interval(self.tick_count);
        let r = stable_random(self.tick_count, EVENT_DEATH_COUNT, self.world_seed);
        let kills = (r % (self.tunables.max_death as u64 + 1)) as u32;
        for kill_idx in 0..kills {
            if let Some(victim) = self.roll_death_victim(kill_idx) {
                self.kill_citizen(victim);
                died += 1;
            }
        }
    }

    // 3. Growth event
    if self.tick_count < self.next_growth_tick {
        return (0, died);
    }
    self.next_growth_tick = self.tick_count + self.roll_growth_interval(self.tick_count);

    let alive = self.citizen_ids.len() as u32;
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
        if self.spawn_citizen().is_none() { break; }
        spawned += 1;
    }
    self.food -= (spawned as i64) * self.tunables.food_per_spawn;
    (spawned, died)
}

    pub fn citizens(&self) -> u32 { self.citizen_ids.len() as u32 }
    pub fn dead_count(&self) -> u32 { self.dead_citizen_ids.len() as u32 }
    pub fn add_food(&mut self, amount: i64) { self.food += amount; }
    pub fn set_cap(&mut self, cap: u32) { self.population_cap = cap; }

    pub fn citizen(&self, i: usize) -> CitizenView {
        CitizenView { id: self.citizen_ids[i], eq: self.citizen_eq[i] }
    }

    pub fn dead_citizen(&self, i: usize) -> DeadCitizenView {
        DeadCitizenView {
            id: self.dead_citizen_ids[i],
            eq: self.dead_citizen_eq[i],
            death_tick: self.dead_citizen_death_tick[i],
        }
    }

    pub fn citizen_display_id(&self, i: usize) -> String {
        format!("{:09}", self.citizen_ids[i])
    }

    pub fn dead_citizen_display_id(&self, i: usize) -> String {
        format!("{:09}", self.dead_citizen_ids[i])
    }

    pub fn save(&self, dir: &str) -> std::io::Result<()> {
        let final_path = Path::new(dir);
        let tmp_path = final_path.with_extension("tmp");

        // Wipe any leftover temp dir from a previous crashed save.
        let _ = fs::remove_dir_all(&tmp_path);
        fs::create_dir_all(&tmp_path)?;

        // ---- meta.json ----
        let meta = Meta {
            version: SAVE_VERSION,
            world_seed: self.world_seed,
            tick_count: self.tick_count,
            food: self.food,
            population_cap: self.population_cap,
            tunables: self.tunables.clone(),
            next_citizen_id: self.next_citizen_id,
            next_growth_tick: self.next_growth_tick,
            next_death_tick: self.next_death_tick,
        };
        write_json(&tmp_path.join("meta.json"), &meta)?;

        // ---- shards ----
        let citizens_dir = tmp_path.join("citizens");
        let alive_dir = citizens_dir.join("alive");
        let dead_dir = citizens_dir.join("dead");
        fs::create_dir_all(&alive_dir)?;
        fs::create_dir_all(&dead_dir)?;

        save_shards(&alive_dir, &self.citizen_ids, &self.citizen_eq, None)?;
        save_shards(
            &dead_dir,
            &self.dead_citizen_ids,
            &self.dead_citizen_eq,
            Some(&self.dead_citizen_death_tick),
        )?;

        // ---- atomic swap: old save survives until this line ----
        let _ = fs::remove_dir_all(final_path);
        fs::rename(&tmp_path, final_path)?;
        Ok(())
    }
    pub fn load(dir: &str) -> std::io::Result<Self> {
        let root = Path::new(dir);
        let meta: Meta = read_json(&root.join("meta.json"))?;
        if meta.version != SAVE_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("save version {} unsupported (expected {})", meta.version, SAVE_VERSION),
            ));
        }

        let citizens_dir = root.join("citizens");
        let (citizen_ids, citizen_eq, _) = load_shards(&citizens_dir.join("alive"))?;
        let (dead_citizen_ids, dead_citizen_eq, dead_citizen_death_tick) =
        load_shards(&citizens_dir.join("dead"))?;
        let n = citizen_ids.len();
        if citizen_eq.len() != n {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("alive columns mismatched: ids={} eq={}", n, citizen_eq.len()),
            ));
        }
        let nd = dead_citizen_ids.len();
        if dead_citizen_eq.len() != nd || dead_citizen_death_tick.len() != nd {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("dead columns mismatched: ids={} eq={} tick={}",
                        nd, dead_citizen_eq.len(), dead_citizen_death_tick.len()),
            ));
        }

        Ok(Sim {
            world_seed: meta.world_seed,
            tick_count: meta.tick_count,
            food: meta.food,
            population_cap: meta.population_cap,
            tunables: meta.tunables,
            citizen_ids,
            citizen_eq,
            next_citizen_id: meta.next_citizen_id,
            next_growth_tick: meta.next_growth_tick,
            next_death_tick: meta.next_death_tick,
            dead_citizen_ids,
            dead_citizen_eq,
            dead_citizen_death_tick,
        })
    }
}

fn save_shards(dir: &Path, ids: &[u32], eq: &[f32], death_tick: Option<&[u64]>) -> std::io::Result<()> {
    let n = ids.len();
    let shard_count = ((n + CITIZENS_PER_SHARD - 1) / CITIZENS_PER_SHARD).max(1);
    for shard_idx in 0..shard_count {
        let start = shard_idx * CITIZENS_PER_SHARD;
        let end = (start + CITIZENS_PER_SHARD).min(n);
        let shard = Shard {
            ids: ids[start..end].to_vec(),
            eq: eq[start..end].to_vec(),
            death_tick: death_tick.map(|d| d[start..end].to_vec()).unwrap_or_default(),
        };
        let shard_dir = dir.join(format!("{:03}", shard_idx));
        fs::create_dir_all(&shard_dir)?;
        write_json(&shard_dir.join("data.json"), &shard)?;
    }
    Ok(())
}

fn load_shards(dir: &Path) -> std::io::Result<(Vec<u32>, Vec<f32>, Vec<u64>)> {
    let mut ids = Vec::new();
    let mut eq = Vec::new();
    let mut death_tick = Vec::new();

    let Ok(entries) = fs::read_dir(dir) else {
        return Ok((ids, eq, death_tick));
    };
    let mut shard_dirs: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    shard_dirs.sort();

    for shard in shard_dirs {
        let s: Shard = read_json(&shard.join("data.json"))?;
        ids.extend(s.ids);
        eq.extend(s.eq);
        death_tick.extend(s.death_tick);
    }
    Ok((ids, eq, death_tick))
}

#[derive(Clone, Copy, Debug)]
pub struct CitizenView { pub id: u32, pub eq: f32 }

#[derive(Clone, Copy, Debug)]
pub struct DeadCitizenView { pub id: u32, pub eq: f32, pub death_tick: u64 }