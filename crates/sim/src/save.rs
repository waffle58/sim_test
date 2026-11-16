use crate::sim::Sim;
use crate::tunables::Tunables;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const SAVE_VERSION: u32 = 10;
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
    next_household_id: u32,
    next_couple_tick: u64,
}

#[derive(Serialize, Deserialize)]
struct Shard {
    ids: Vec<u32>,
    eq: Vec<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    base_eq: Vec<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    native_born: Vec<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    aptitude: Vec<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    female: Vec<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    death_tick: Vec<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    household_id: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    surname_from: Vec<u32>,
}
#[derive(Serialize, Deserialize)]
struct HouseholdShard {
    ids: Vec<u32>,
    member_starts: Vec<u32>,
    member_ids: Vec<u32>,
}


struct ColumnarFormatter;
impl serde_json::ser::Formatter for ColumnarFormatter {
    fn begin_array<W: ?Sized + std::io::Write>(&mut self, w: &mut W) -> std::io::Result<()> { w.write_all(b"[") }
    fn end_array<W: ?Sized + std::io::Write>(&mut self, w: &mut W) -> std::io::Result<()> { w.write_all(b"]") }
    fn begin_array_value<W: ?Sized + std::io::Write>(&mut self, w: &mut W, first: bool) -> std::io::Result<()> {
        if !first { w.write_all(b",")?; } Ok(())
    }
    fn begin_object<W: ?Sized + std::io::Write>(&mut self, w: &mut W) -> std::io::Result<()> { w.write_all(b"{") }
    fn end_object<W: ?Sized + std::io::Write>(&mut self, w: &mut W) -> std::io::Result<()> { w.write_all(b"\n}") }
    fn begin_object_key<W: ?Sized + std::io::Write>(&mut self, w: &mut W, first: bool) -> std::io::Result<()> {
        if first { w.write_all(b"\n  ")?; } else { w.write_all(b",\n  ")?; } Ok(())
    }
    fn begin_object_value<W: ?Sized + std::io::Write>(&mut self, w: &mut W) -> std::io::Result<()> {
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

fn save_shards(
    dir: &Path,
    ids: &[u32],
    eq: &[f32],
    base_eq: &[f32],
    native_born: &[bool],
    aptitude: &[f32],
    female: &[bool],
    death_tick: Option<&[u64]>,
    household_id: &[u32],
    surname_from: &[u32],
) -> std::io::Result<()> {
    let n = ids.len();
    let shard_count = ((n + CITIZENS_PER_SHARD - 1) / CITIZENS_PER_SHARD).max(1);
    for shard_idx in 0..shard_count {
        let start = shard_idx * CITIZENS_PER_SHARD;
        let end = (start + CITIZENS_PER_SHARD).min(n);
        let shard = Shard {
            ids: ids[start..end].to_vec(),
            eq: eq[start..end].to_vec(),
            base_eq: base_eq.get(start..end).map(|s| s.to_vec()).unwrap_or_default(),
            native_born: native_born.get(start..end).map(|s| s.to_vec()).unwrap_or_default(),
            aptitude: aptitude.get(start..end).map(|s| s.to_vec()).unwrap_or_default(),
            female: female.get(start..end).map(|s| s.to_vec()).unwrap_or_default(),
            death_tick: death_tick.map(|d| d[start..end].to_vec()).unwrap_or_default(),
            household_id: household_id.get(start..end).map(|s| s.to_vec()).unwrap_or_default(),
            surname_from: surname_from.get(start..end).map(|s| s.to_vec()).unwrap_or_default(),
        };
        let shard_dir = dir.join(format!("{:03}", shard_idx));
        fs::create_dir_all(&shard_dir)?;
        write_json(&shard_dir.join("data.json"), &shard)?;
    }
    Ok(())
}

struct LoadedShards {
    ids: Vec<u32>,
    eq: Vec<f32>,
    base_eq: Vec<f32>,
    native_born: Vec<bool>,
    aptitude: Vec<f32>,
    female: Vec<bool>,
    death_tick: Vec<u64>,
    household_id: Vec<u32>,
    surname_from: Vec<u32>,
}

fn load_shards(dir: &Path) -> std::io::Result<LoadedShards> {
    let mut out = LoadedShards {
        ids: Vec::new(),
        eq: Vec::new(),
        base_eq: Vec::new(),
        native_born: Vec::new(),
        aptitude: Vec::new(),
        female: Vec::new(),
        death_tick: Vec::new(),
        household_id: Vec::new(),
        surname_from: Vec::new(),
    };

    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(out);
    };
    let mut shard_dirs: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    shard_dirs.sort();

    for shard in shard_dirs {
        let s: Shard = read_json(&shard.join("data.json"))?;
        out.ids.extend(s.ids);
        out.eq.extend(s.eq);
        out.base_eq.extend(s.base_eq);
        out.native_born.extend(s.native_born);
        out.aptitude.extend(s.aptitude);
        out.female.extend(s.female);
        out.death_tick.extend(s.death_tick);
        out.household_id.extend(s.household_id);
        out.surname_from.extend(s.surname_from);
    }
    Ok(out)
}

impl Sim {
    pub fn save(&self, dir: &str) -> std::io::Result<()> {
        let final_path = Path::new(dir);
        let tmp_path = final_path.with_extension("tmp");

        let _ = fs::remove_dir_all(&tmp_path);
        fs::create_dir_all(&tmp_path)?;

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
            next_household_id: self.next_household_id,
            next_couple_tick: self.next_couple_tick,
        };
        write_json(&tmp_path.join("meta.json"), &meta)?;

        let citizens_dir = tmp_path.join("citizens");
        let alive_dir = citizens_dir.join("alive");
        let dead_dir = citizens_dir.join("dead");
        fs::create_dir_all(&alive_dir)?;
        fs::create_dir_all(&dead_dir)?;

        save_shards(
            &alive_dir,
            &self.civ_ids,
            &self.civ_eq,
            &self.civ_base_eq,
            &self.civ_native_born,
            &self.civ_aptitude,
            &self.civ_female,
            None,
            &self.civ_household,
            &self.civ_surname_from,
        )?;
        save_shards(
            &dead_dir,
            &self.dead_civ_ids,
            &self.dead_civ_eq,
            &[],
            &[],
            &[],
            &[],
            Some(&self.dead_civ_death_tick),
            &[],
            &[],
        )?;
        let households_dir = tmp_path.join("households");
        fs::create_dir_all(&households_dir)?;
        let household_shard = HouseholdShard {
            ids: self.household_ids.clone(),
            member_starts: self.household_member_starts.clone(),
            member_ids: self.household_member_ids.clone(),
        };
        write_json(&households_dir.join("data.json"), &household_shard)?;

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
        let alive = load_shards(&citizens_dir.join("alive"))?;
        let dead = load_shards(&citizens_dir.join("dead"))?;
        let household_shard: HouseholdShard = read_json(&root.join("households/data.json"))?;

        if household_shard.member_starts.len() != household_shard.ids.len() + 1 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                format!("household member_starts length {} != ids length {} + 1",
                        household_shard.member_starts.len(), household_shard.ids.len())));
        }
        if household_shard.member_starts.last().copied().unwrap_or(0) as usize != household_shard.member_ids.len() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                "household member_starts.last != member_ids.len"));
        }
        for i in 0..household_shard.ids.len() {
            if household_shard.member_starts[i + 1] < household_shard.member_starts[i] {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                    format!("household member_starts not monotonic at {}", i)));
            }
            if household_shard.member_starts[i + 1] == household_shard.member_starts[i] {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                    format!("empty household {} in save", household_shard.ids[i])));
            }
        }

        let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for &civ_id in &household_shard.member_ids {
            if !seen.insert(civ_id) {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                    format!("civ {} appears in multiple households", civ_id)));
            }
        }
        if seen.len() != alive.ids.len() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                format!("{} civs but {} in households", alive.ids.len(), seen.len())));
        }
        let n = alive.ids.len();
        if alive.eq.len() != n
            || alive.base_eq.len() != n
            || alive.native_born.len() != n
            || alive.aptitude.len() != n
            || alive.female.len() != n
            || alive.surname_from.len() != n
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "alive columns mismatched: ids={} eq={} base_eq={} native={} apt={} female={}",
                    n, alive.eq.len(), alive.base_eq.len(), alive.native_born.len(),
                    alive.aptitude.len(), alive.female.len()
                ),
            ));
        }
        let nd = dead.ids.len();
        if dead.eq.len() != nd || dead.death_tick.len() != nd {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("dead columns mismatched: ids={} eq={} tick={}",
                        nd, dead.eq.len(), dead.death_tick.len()),
            ));
        }

        Ok(Sim {
            world_seed: meta.world_seed,
            tick_count: meta.tick_count,
            food: meta.food,
            population_cap: meta.population_cap,
            tunables: meta.tunables,
            civ_ids: alive.ids,
            civ_eq: alive.eq,
            civ_base_eq: alive.base_eq,
            civ_native_born: alive.native_born,
            civ_aptitude: alive.aptitude,
            civ_female: alive.female,
            next_civ_id: meta.next_civ_id,
            next_growth_tick: meta.next_growth_tick,
            next_death_tick: meta.next_death_tick,
            dead_civ_ids: dead.ids,
            dead_civ_eq: dead.eq,
            dead_civ_death_tick: dead.death_tick,
            total_deaths: meta.total_deaths,
            civ_household: alive.household_id,
            household_ids: household_shard.ids,
            household_member_starts: household_shard.member_starts,
            household_member_ids: household_shard.member_ids,
            next_household_id: meta.next_household_id,
            next_couple_tick: meta.next_couple_tick,
            civ_surname_from: alive.surname_from,
        })
    }
}