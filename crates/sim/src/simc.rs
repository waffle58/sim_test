//! The simulation state and its tick loop.

use crate::random::stable_random;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// --- Population growth ---
const STARTING_CITIZENS: u32 = 10;
pub const DEFAULT_POPULATION_CAP: u32 = 50;
const MIN_GROWTH_INTERVAL: u64 = 35; 
const MAX_GROWTH_INTERVAL: u64 = 45; 
const MAX_GROWTH: u32 = 3;
const EVENT_POP_GROWTH: u64 = 1;
const EVENT_POP_INTERVAL: u64 = 2;

// --- Food ---
const STARTING_FOOD: i64 = 100;
const FOOD_PER_SPAWN: i64 = 10;

// --- Per-citizen eq drift ---
const EQ_STEP: f32 = 0.02;
const EQ_DRIFT_PERIOD: u64 = 17;
const EVENT_INITIAL_EQ: u64 = 3;
const EVENT_EQ_TRIGGER: u64 = 4;
const EVENT_EQ_DRIFT: u64 = 5;

// --- Death ---
// Separate timer from growth. Interval rolls independently.
const MIN_DEATH_INTERVAL: u64 = 95;    
const MAX_DEATH_INTERVAL: u64 = 105;   
const MAX_DEATH: u32 = 2;
const EVENT_DEATH_INTERVAL: u64 = 6;
const EVENT_DEATH_COUNT: u64 = 7;
const EVENT_DEATH_PICK: u64 = 8;

// --- Public code generation ---
const ID_BITS: u32 = 50;
const TYPE_SHIFT: u32 = 40;
const INTERNAL_MASK: u64 = (1u64 << TYPE_SHIFT) - 1;
const ID_MASK: u64 = (1u64 << ID_BITS) - 1;
const CODE_LEN: usize = 10;
const EVENT_ID_KEY: u64 = 100;
const TYPE_CITIZEN: u8 = 1;
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

// --- Save format ---
const SAVE_VERSION: u32 = 3;
const CITIZENS_PER_SHARD: usize = 10_000;

#[derive(Serialize, Deserialize)]
pub struct Sim {
    pub world_seed: u64,
    pub tick_count: u64,

    pub food: i64,
    pub population_cap: u32,

    // Alive citizens (SoA).
    pub citizen_ids: Vec<u32>,
    pub citizen_eq: Vec<f32>,
    pub next_citizen_id: u32,
    next_growth_tick: u64,
    next_death_tick: u64,

    // Dead citizens (SoA). No removal from these — history stays.
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

fn derive_odd_key(seed: u64) -> u64 {
    (stable_random(0, EVENT_ID_KEY, seed) | 1) & ID_MASK
}

fn public_code_u64(internal_id: u32, type_tag: u8, seed: u64) -> u64 {
    let odd_key = derive_odd_key(seed);
    let packed = ((type_tag as u64) << TYPE_SHIFT) | (internal_id as u64 & INTERNAL_MASK);
    (packed.wrapping_mul(odd_key)) & ID_MASK
}

fn encode_code(mut n: u64, len: usize) -> String {
    let mut out = vec![b'0'; len];
    for i in (0..len).rev() {
        out[i] = CROCKFORD[(n & 31) as usize];
        n >>= 5;
    }
    String::from_utf8(out).expect("crockford is ascii")
}

fn quantize_eq(x: f32) -> f32 {
    (x * 100.0).round() / 100.0
}

impl Sim {
    pub fn new(world_seed: u64) -> Self {
        let mut sim = Sim {
            world_seed,
            tick_count: 0,
            food: STARTING_FOOD,
            population_cap: DEFAULT_POPULATION_CAP,
            citizen_ids: Vec::new(),
            citizen_eq: Vec::new(),
            next_citizen_id: 0,
            next_growth_tick: 0,
            next_death_tick: 0,
            dead_citizen_ids: Vec::new(),
            dead_citizen_eq: Vec::new(),
            dead_citizen_death_tick: Vec::new(),
        };
        for _ in 0..STARTING_CITIZENS {
            sim.spawn_citizen();
        }
        sim.next_growth_tick = sim.tick_count + sim.roll_growth_interval(sim.tick_count);
        sim.next_death_tick = sim.tick_count + sim.roll_death_interval(sim.tick_count);
        sim
    }

    pub fn spawn_citizen(&mut self) -> u32 {
        let id = self.next_citizen_id;
        self.next_citizen_id += 1;

        let r = stable_random(id as u64, EVENT_INITIAL_EQ, self.world_seed);
        let eq = (r % 100) as f32 / 100.0;

        self.citizen_ids.push(id);
        self.citizen_eq.push(eq);
        id
    }

    /// Move citizen at index `idx` to the dead columns.
    /// Uses swap_remove so the alive arrays stay dense.
    fn kill_citizen(&mut self, idx: usize) {
        let id = self.citizen_ids.swap_remove(idx);
        let eq = self.citizen_eq.swap_remove(idx);
        self.dead_citizen_ids.push(id);
        self.dead_citizen_eq.push(eq);
        self.dead_citizen_death_tick.push(self.tick_count);
    }

    /// Selection rule for who dies. Today: random alive citizen.
    /// Later: replace the body with decision-based selection
    /// (weighted by eq, age, health, etc.). Signature stays the same.
    fn roll_death_victim(&self, kill_idx: u32) -> Option<usize> {
        let n = self.citizen_ids.len();
        if n == 0 { return None; }
        let key = (kill_idx as u64) ^ (self.tick_count << 8);
        let r = stable_random(key, EVENT_DEATH_PICK, self.world_seed);
        Some((r as usize) % n)
    }

    fn roll_growth_interval(&self, at_tick: u64) -> u64 {
        let r = stable_random(at_tick, EVENT_POP_INTERVAL, self.world_seed);
        let span = MAX_GROWTH_INTERVAL - MIN_GROWTH_INTERVAL + 1;
        MIN_GROWTH_INTERVAL + r % span
    }

    fn roll_death_interval(&self, at_tick: u64) -> u64 {
        let r = stable_random(at_tick, EVENT_DEATH_INTERVAL, self.world_seed);
        let span = MAX_DEATH_INTERVAL - MIN_DEATH_INTERVAL + 1;
        MIN_DEATH_INTERVAL + r % span
    }

    pub fn tick(&mut self) -> (u32, u32) {
    self.tick_count += 1;
    let mut died = 0u32;

    // 1. Per-citizen eq drift (unchanged body)
    for i in 0..self.citizen_ids.len() {
        let key = (self.citizen_ids[i] as u64) ^ (self.tick_count << 32);
        if stable_random(key, EVENT_EQ_TRIGGER, self.world_seed) % EQ_DRIFT_PERIOD != 0 {
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
        let kills = (r % (MAX_DEATH as u64 + 1)) as u32;
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
    let wanted = (r % (MAX_GROWTH as u64 + 1)) as u32;
    let room = self.population_cap - alive;
    let affordable = if self.food <= 0 { 0 } else { (self.food / FOOD_PER_SPAWN) as u32 };
    let actual = wanted.min(room).min(affordable);

    for _ in 0..actual {
        self.spawn_citizen();
    }
    self.food -= (actual as i64) * FOOD_PER_SPAWN;
    (actual, died)
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

    pub fn citizen_code(&self, i: usize) -> String {
        encode_code(public_code_u64(self.citizen_ids[i], TYPE_CITIZEN, self.world_seed), CODE_LEN)
    }

    pub fn dead_citizen_code(&self, i: usize) -> String {
        encode_code(public_code_u64(self.dead_citizen_ids[i], TYPE_CITIZEN, self.world_seed), CODE_LEN)
    }

    pub fn save(&self, dir: &str) -> std::io::Result<()> {
        let root = Path::new(dir);
        let _ = fs::remove_dir_all(root);
        fs::create_dir_all(root)?;

        let meta = Meta {
            version: SAVE_VERSION,
            world_seed: self.world_seed,
            tick_count: self.tick_count,
            food: self.food,
            population_cap: self.population_cap,
            next_citizen_id: self.next_citizen_id,
            next_growth_tick: self.next_growth_tick,
            next_death_tick: self.next_death_tick,
        };
        write_json(&root.join("meta.json"), &meta)?;

        let citizens_dir = root.join("citizens");
        let alive_dir = citizens_dir.join("alive");
        let dead_dir = citizens_dir.join("dead");
        fs::create_dir_all(&alive_dir)?;
        fs::create_dir_all(&dead_dir)?;

        save_shards(&alive_dir, &self.citizen_ids, &self.citizen_eq, None)?;
        save_shards(&dead_dir, &self.dead_citizen_ids, &self.dead_citizen_eq,
                    Some(&self.dead_citizen_death_tick))?;
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

        Ok(Sim {
            world_seed: meta.world_seed,
            tick_count: meta.tick_count,
            food: meta.food,
            population_cap: meta.population_cap,
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
        return Ok((ids, eq, death_tick));   // missing folder = empty
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