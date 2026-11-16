use serde::{Deserialize, Serialize};
use std::fs;

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
    pub max_dead_retained: u32,
    pub initial_couple_percent: u32,
    pub couple_rate_percent: u32,
    pub min_couple_interval: u64,
    pub max_couple_interval: u64,
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
            max_dead_retained: 10_000,
            initial_couple_percent: 60,
            couple_rate_percent: 5,
            min_couple_interval: 100,
            max_couple_interval: 200,
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

    pub fn set_field(&mut self, field: &str, value: &str) -> Result<(), String> {
        let parse_u64 = |v: &str| v.parse::<u64>().map_err(|_| format!("invalid number '{}'", v));
        let parse_i64 = |v: &str| v.parse::<i64>().map_err(|_| format!("invalid number '{}'", v));
        let parse_u32 = |v: &str| v.parse::<u32>().map_err(|_| format!("invalid number '{}'", v));
        if self.min_couple_interval > self.max_couple_interval {
        return Err(format!(
        "min_couple_interval ({}) > max_couple_interval ({})",
        self.min_couple_interval, self.max_couple_interval));
}

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
            "max_dead_retained"    => self.max_dead_retained = parse_u32(value)?,
            "initial_couple_percent" => self.initial_couple_percent = parse_u32(value)?,
            "couple_rate_percent"    => self.couple_rate_percent = parse_u32(value)?,
            "min_couple_interval"    => self.min_couple_interval = parse_u64(value)?,
            "max_couple_interval"    => self.max_couple_interval = parse_u64(value)?,
            other => return Err(format!("unknown field '{}'", other)),
        }

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
            format!("max_dead_retained    = {}", self.max_dead_retained),
            format!("initial_couple_percent = {}", self.initial_couple_percent),
            format!("couple_rate_percent    = {}", self.couple_rate_percent),
            format!("min_couple_interval    = {} ticks ({:.2}s)", self.min_couple_interval, self.min_couple_interval as f64 / 20.0),
            format!("max_couple_interval    = {} ticks ({:.2}s)", self.max_couple_interval, self.max_couple_interval as f64 / 20.0),
        ]
    }
}