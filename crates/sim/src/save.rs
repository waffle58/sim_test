use crate::sim::Sim;
use crate::tunables::Tunables;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const SAVE_VERSION: u32 = 6;
const CITIZENS_PER_SHARD: usize = 10_000;

#[derive(Serialize, Deserialize)]
struct Meta {
    version: u32,
    world_seed: u64,
    tick_count: u64,
    food: i64,
    population_cap: u32,
    tunables: Tunables,
    next_civ_id: u32,
    next_growth_tick: u64,
    next_death_tick: u64,
    total_deaths: u64,
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
    fn begin_array<W>(&mut self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        w.write_all(b"[")
    }
    fn end_array<W>(&mut self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        w.write_all(b"]")
    }
    fn begin_array_value<W>(&mut self, w: &mut W, first: bool) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        if !first {
            w.write_all(b",")?;
        }
        Ok(())
    }
    fn begin_object<W>(&mut self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        w.write_all(b"{")
    }
    fn end_object<W>(&mut self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        w.write_all(b"\n}")
    }
    fn begin_object_key<W>(&mut self, w: &mut W, first: bool) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        if first {
            w.write_all(b"\n  ")?;
        } else {
            w.write_all(b",\n  ")?;
        }
        Ok(())
    }
    fn begin_object_value<W>(&mut self, w: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        w.write_all(b": ")
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, ColumnarFormatter);
    value
        .serialize(&mut ser)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(path, buf)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> std::io::Result<T> {
    let s = fs::read_to_string(path)?;
    serde_json::from_str(&s).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn save_shards(
    dir: &Path,
    ids: &[u32],
    eq: &[f32],
    death_tick: Option<&[u64]>,
) -> std::io::Result<()> {
    let n = ids.len();
    let shard_count = ((n + CITIZENS_PER_SHARD - 1) / CITIZENS_PER_SHARD).max(1);
    for shard_idx in 0..shard_count {
        let start = shard_idx * CITIZENS_PER_SHARD;
        let end = (start + CITIZENS_PER_SHARD).min(n);
        let shard = Shard {
            ids: ids[start..end].to_vec(),
            eq: eq[start..end].to_vec(),
            death_tick: death_tick
                .map(|d| d[start..end].to_vec())
                .unwrap_or_default(),
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

impl Sim {
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
            next_civ_id: self.next_civ_id,
            next_growth_tick: self.next_growth_tick,
            next_death_tick: self.next_death_tick,
            total_deaths: self.total_deaths,
        };
        write_json(&tmp_path.join("meta.json"), &meta)?;

        // ---- shards ----
        let citizens_dir = tmp_path.join("citizens");
        let alive_dir = citizens_dir.join("alive");
        let dead_dir = citizens_dir.join("dead");
        fs::create_dir_all(&alive_dir)?;
        fs::create_dir_all(&dead_dir)?;

        save_shards(&alive_dir, &self.civ_ids, &self.civ_eq, None)?;
        save_shards(
            &dead_dir,
            &self.dead_civ_ids,
            &self.dead_civ_eq,
            Some(&self.dead_civ_death_tick),
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
                format!(
                    "save version {} unsupported (expected {})",
                    meta.version, SAVE_VERSION
                ),
            ));
        }

        let citizens_dir = root.join("citizens");
        let (citizen_ids, citizen_eq, _) = load_shards(&citizens_dir.join("alive"))?;
        let (dead_citizen_ids, dead_civ_eq, dead_citizen_death_tick) =
            load_shards(&citizens_dir.join("dead"))?;
        let n = citizen_ids.len();
        if citizen_eq.len() != n {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "alive columns mismatched: ids={} eq={}",
                    n,
                    citizen_eq.len()
                ),
            ));
        }
        let nd = dead_citizen_ids.len();
        if dead_civ_eq.len() != nd || dead_citizen_death_tick.len() != nd {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "dead columns mismatched: ids={} eq={} tick={}",
                    nd,
                    dead_civ_eq.len(),
                    dead_citizen_death_tick.len()
                ),
            ));
        }

        Ok(Sim {
            world_seed: meta.world_seed,
            tick_count: meta.tick_count,
            food: meta.food,
            population_cap: meta.population_cap,
            tunables: meta.tunables,
            civ_ids: citizen_ids,
            civ_eq: citizen_eq,
            next_civ_id: meta.next_civ_id,
            next_growth_tick: meta.next_growth_tick,
            next_death_tick: meta.next_death_tick,
            dead_civ_ids: dead_citizen_ids,
            dead_civ_eq: dead_civ_eq,
            dead_civ_death_tick: dead_citizen_death_tick,
            total_deaths: meta.total_deaths,
        })
    }
}