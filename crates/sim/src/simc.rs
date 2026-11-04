//! The simulation state and its tick loop.

use crate::random::stable_random;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// --- Population growth ---
const STARTING_CITIZENS: u32 = 10;
pub const POPULATION_CAP: u32 = 50;
const MIN_GROWTH_INTERVAL: u64 = 1;
const MAX_GROWTH_INTERVAL: u64 = 5;
const MAX_GROWTH: u32 = 3;
const EVENT_POP_GROWTH: u64 = 1;
const EVENT_POP_INTERVAL: u64 = 2;

// --- Per-citizen eq drift ---
const EQ_STEP: f32 = 0.02;
const EQ_DRIFT_PERIOD: u64 = 17;   // 1 in 17 ticks per citizen, on average
const EVENT_INITIAL_EQ: u64 = 3;
const EVENT_EQ_TRIGGER: u64 = 4;
const EVENT_EQ_DRIFT: u64 = 5;

// --- Save format ---
const SAVE_VERSION: u32 = 2;
const CITIZENS_PER_SHARD: usize = 10_000;

// --- Public code generation ---
const ID_BITS: u32 = 50;
const TYPE_SHIFT: u32 = 40;
const INTERNAL_MASK: u64 = (1u64 << TYPE_SHIFT) - 1;
const ID_MASK: u64 = (1u64 << ID_BITS) - 1;
const CODE_LEN: usize = 10;
const EVENT_ID_KEY: u64 = 100;

const TYPE_CITIZEN: u8 = 1;

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

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

#[derive(Serialize, Deserialize)]
pub struct Sim {
    pub world_seed: u64,
    pub tick_count: u64,

    // Citizens (SoA). Index i in each column is the same citizen.
    pub citizen_ids: Vec<u32>,
    pub citizen_eq: Vec<f32>,
    pub next_citizen_id: u32,

    next_growth_tick: u64,
}

#[derive(Serialize, Deserialize)]
struct Meta {
    version: u32,
    world_seed: u64,
    tick_count: u64,
    next_citizen_id: u32,
    next_growth_tick: u64,
}

#[derive(Serialize, Deserialize)]
struct Shard {
    ids: Vec<u32>,
    eq: Vec<f32>,
}

/// Pretty objects, compact arrays — one line per category, values packed.
struct ColumnarFormatter;

impl serde_json::ser::Formatter for ColumnarFormatter {
    fn begin_array<W>(&mut self, w: &mut W) -> std::io::Result<()> where W: ?Sized + std::io::Write {
        w.write_all(b"[")
    }
    fn end_array<W>(&mut self, w: &mut W) -> std::io::Result<()> where W: ?Sized + std::io::Write {
        w.write_all(b"]")
    }
    fn begin_array_value<W>(&mut self, w: &mut W, first: bool) -> std::io::Result<()> where W: ?Sized + std::io::Write {
        if !first { w.write_all(b",")?; }
        Ok(())
    }
    fn begin_object<W>(&mut self, w: &mut W) -> std::io::Result<()> where W: ?Sized + std::io::Write {
        w.write_all(b"{")
    }
    fn end_object<W>(&mut self, w: &mut W) -> std::io::Result<()> where W: ?Sized + std::io::Write {
        w.write_all(b"\n}")
    }
    fn begin_object_key<W>(&mut self, w: &mut W, first: bool) -> std::io::Result<()> where W: ?Sized + std::io::Write {
        if first { w.write_all(b"\n  ")?; } else { w.write_all(b",\n  ")?; }
        Ok(())
    }
    fn begin_object_value<W>(&mut self, w: &mut W) -> std::io::Result<()> where W: ?Sized + std::io::Write {
        w.write_all(b": ")
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, ColumnarFormatter);
    value.serialize(&mut ser)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(path, buf)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> std::io::Result<T> {
    let s = fs::read_to_string(path)?;
    serde_json::from_str(&s)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

impl Sim {
    pub fn new(world_seed: u64) -> Self {
        let mut sim = Sim {
            world_seed,
            tick_count: 0,
            citizen_ids: Vec::new(),
            citizen_eq: Vec::new(),
            next_citizen_id: 0,
            next_growth_tick: 0,
        };
        for _ in 0..STARTING_CITIZENS {
            sim.spawn_citizen();
        }
        sim.next_growth_tick = sim.tick_count + sim.roll_growth_interval(sim.tick_count);
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

    fn roll_growth_interval(&self, at_tick: u64) -> u64 {
        let r = stable_random(at_tick, EVENT_POP_INTERVAL, self.world_seed);
        let span = MAX_GROWTH_INTERVAL - MIN_GROWTH_INTERVAL + 1;
        MIN_GROWTH_INTERVAL + r % span
    }

    pub fn tick(&mut self) -> u32 {
        self.tick_count += 1;

        // Eq drift: each citizen has a 1-in-EQ_DRIFT_PERIOD chance per tick.
        // The (id, tick) key staggers updates so they are unsynchronized.
        for i in 0..self.citizen_ids.len() {
            let key = (self.citizen_ids[i] as u64) ^ (self.tick_count << 32);

            if stable_random(key, EVENT_EQ_TRIGGER, self.world_seed) % EQ_DRIFT_PERIOD != 0 {
                continue;
            }

            let roll = stable_random(key, EVENT_EQ_DRIFT, self.world_seed) % 3;
            let delta = match roll {
                0 => -EQ_STEP,
                1 => 0.0,
                _ => EQ_STEP,
            };
            self.citizen_eq[i] = quantize_eq((self.citizen_eq[i] + delta).clamp(0.0, 1.0));
        }

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

    pub fn citizen(&self, i: usize) -> CitizenView {
        CitizenView {
            id: self.citizen_ids[i],
            eq: self.citizen_eq[i],
        }
    }
    pub fn citizen_code(&self, i: usize) -> String {
        encode_code(
            public_code_u64(self.citizen_ids[i], TYPE_CITIZEN, self.world_seed),
            CODE_LEN,
        )
    }
    pub fn save(&self, dir: &str) -> std::io::Result<()> {
        let root = Path::new(dir);
        let _ = fs::remove_dir_all(root);
        fs::create_dir_all(root)?;

        let meta = Meta {
            version: SAVE_VERSION,
            world_seed: self.world_seed,
            tick_count: self.tick_count,
            next_citizen_id: self.next_citizen_id,
            next_growth_tick: self.next_growth_tick,
        };
        write_json(&root.join("meta.json"), &meta)?;

        let citizens_dir = root.join("citizens");
        fs::create_dir_all(&citizens_dir)?;

        let n = self.citizen_ids.len();
        let shard_count = ((n + CITIZENS_PER_SHARD - 1) / CITIZENS_PER_SHARD).max(1);
        for shard_idx in 0..shard_count {
            let start = shard_idx * CITIZENS_PER_SHARD;
            let end = (start + CITIZENS_PER_SHARD).min(n);
            let shard = Shard {
                ids: self.citizen_ids[start..end].to_vec(),
                eq: self.citizen_eq[start..end].to_vec(),
            };
            let shard_dir = citizens_dir.join(format!("{:03}", shard_idx));
            fs::create_dir_all(&shard_dir)?;
            write_json(&shard_dir.join("data.json"), &shard)?;
        }
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
        let mut shard_dirs: Vec<_> = fs::read_dir(&citizens_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        shard_dirs.sort();

        let mut citizen_ids = Vec::new();
        let mut citizen_eq = Vec::new();

        for shard in shard_dirs {
            let s: Shard = read_json(&shard.join("data.json"))?;
            citizen_ids.extend(s.ids);
            citizen_eq.extend(s.eq);
        }

        Ok(Sim {
            world_seed: meta.world_seed,
            tick_count: meta.tick_count,
            citizen_ids,
            citizen_eq,
            next_citizen_id: meta.next_citizen_id,
            next_growth_tick: meta.next_growth_tick,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CitizenView {
    pub id: u32,
    pub eq: f32,
}