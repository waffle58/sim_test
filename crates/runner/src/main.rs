use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use sim::{Sim, TICKS_PER_SECOND};

const SAVE_DIR: &str = "saves";

enum Command {
    New { name: String, seed: u64 },
    Load { name: String },
    Save,
    Pause,
    Resume,
    Status,
    List { dead: bool },
    AddFood { amount: i64 },
    SetCap { amount: u32 },
    Help,
    Quit,
    Unknown(String),
}

fn derive_seed_from_name(name: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in name.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn parse_command(line: &str) -> Command {
    let mut parts = line.split_whitespace();
    let Some(head) = parts.next() else {
        return Command::Unknown(String::new());
    };
    match head.to_ascii_lowercase().as_str() {
        "new" => {
            let name = parts.next().unwrap_or("world").to_string();
            let seed = parts.next().and_then(|s| s.parse().ok())
                .unwrap_or_else(|| derive_seed_from_name(&name));
            Command::New { name, seed }
        }
        "load" => Command::Load {
            name: parts.next().unwrap_or("world").to_string(),
        },
        "save" => Command::Save,
        "pause" => Command::Pause,
        "resume" | "play" => Command::Resume,
        "status" => Command::Status,
        "list" | "ls" => Command::List { dead: parts.next() == Some("dead") },
        "add" => {
            let what = parts.next();
            let amount = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            match what {
                Some("food") => Command::AddFood { amount },
                _ => Command::Unknown(format!("add {:?}", what)),
            }
        }
        "cap" => Command::SetCap {
            amount: parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        },
        "help" | "?" => Command::Help,
        "quit" | "exit" | "q" => Command::Quit,
        other => Command::Unknown(other.to_string()),
    }
}

fn save_path(name: &str) -> PathBuf {
    PathBuf::from(SAVE_DIR).join(name)
}

fn validate_save_name(name: &str) -> Result<(), String> {
    if name.is_empty() { return Err("save name is empty".into()); }
    if name.len() > 32 { return Err("save name is longer than 32 characters".into()); }
    if name == "." || name == ".." { return Err("'.' and '..' are not valid save names".into()); }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(format!(
            "save name '{}' contains invalid characters (use letters, digits, _ or -)", name));
    }
    Ok(())
}

fn spawn_input_thread() -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) => if tx.send(l).is_err() { break; },
                Err(_) => break,
            }
        }
    });
    rx
}

fn prompt() {
    print!("> ");
    let _ = io::stdout().flush();
}

fn print_help() {
    println!("commands:");
    println!("  new <name> [seed]      start a fresh world (default seed derived from name)");
    println!("  load <name>            read saves/<name>/");
    println!("  save                   write current world to saves/<name>/");
    println!("  pause / resume         stop or start simulation time");
    println!("  status                 world summary (alive, dead, food, cap)");
    println!("  list [dead]            print alive citizens (or dead ones)");
    println!("  add food <n>           add n to the food stockpile");
    println!("  cap <n>                set population cap to n");
    println!("  help                   this list");
    println!("  quit                   exit");
}

fn main() {
    if let Err(e) = std::fs::create_dir_all(SAVE_DIR) {
        eprintln!("could not create {}: {e}", SAVE_DIR);
        return;
    }

    println!("city-sim console. 'new <name>' or 'load <name>' to begin.");
    print_help();
    prompt();

    let commands = spawn_input_thread();
    let tick_len = Duration::from_secs_f64(1.0 / TICKS_PER_SECOND);
    let mut last = Instant::now();
    let mut leftover = Duration::ZERO;

    let mut sim: Option<Sim> = None;
    let mut current_name: Option<String> = None;
    let mut paused = false;

    loop {
        let mut quit = false;
        loop {
            match commands.try_recv() {
                Ok(line) => {
                    match parse_command(&line) {
                        Command::Quit => { quit = true; break; }
                        Command::Help => print_help(),
                        Command::New { name, seed } => {
                            if let Err(e) = validate_save_name(&name) {
                                eprintln!("{}", e);
                            } else {
                                let s = Sim::new(seed);
                                println!("new '{}' (seed {}), alive={} food={}",
                                         name, seed, s.citizens(), s.food);
                                sim = Some(s);
                                current_name = Some(name);
                                paused = false;
                            }
                        }
                        Command::Load { name } => {
                            if let Err(e) = validate_save_name(&name) {
                                eprintln!("{}", e);
                            } else {
                                match Sim::load(save_path(&name).to_str().unwrap()) {
                                    Ok(s) => {
                                        println!("loaded '{}', alive={} dead={} food={} t={}",
                                                 name, s.citizens(), s.dead_count(), s.food, s.tick_count);
                                        sim = Some(s);
                                        current_name = Some(name);
                                        paused = false;
                                    }
                                    Err(e) => eprintln!("load failed: {e}"),
                                }
                            }
                        }
                        Command::Save => match (&sim, &current_name) {
                            (Some(s), Some(name)) => {
                                match s.save(save_path(name).to_str().unwrap()) {
                                    Ok(()) => println!("saved '{}' -> {}", name, save_path(name).display()),
                                    Err(e) => eprintln!("save failed: {e}"),
                                }
                            }
                            _ => eprintln!("no world loaded"),
                        },
                        Command::Pause => {
                            if sim.is_some() { paused = true; println!("paused"); }
                            else { eprintln!("no world loaded"); }
                        }
                        Command::Resume => {
                            if sim.is_some() { paused = false; println!("resumed"); }
                            else { eprintln!("no world loaded"); }
                        }
                        Command::Status => match (&sim, &current_name) {
                            (Some(s), Some(name)) => println!(
                                "'{}' seed={} alive={}/{} dead={} food={} t={} {}",
                                name, s.world_seed, s.citizens(), s.population_cap,
                                s.dead_count(), s.food, s.tick_count,
                                if paused { "(paused)" } else { "(running)" }
                            ),
                            _ => println!("no world loaded"),
                        },
                        Command::List { dead } => match &sim {
                            Some(s) => {
                                if dead {
                                    println!("{} dead citizens in '{}':",
                                             s.dead_count(), current_name.as_deref().unwrap_or("?"));
                                    for i in 0..s.dead_count() as usize {
                                        let d = s.dead_citizen(i);
                                        println!("  {} eq={:.2} died_t={}",
                                                 s.dead_citizen_code(i), d.eq, d.death_tick);
                                    }
                                } else {
                                    println!("{} alive citizens in '{}':",
                                             s.citizens(), current_name.as_deref().unwrap_or("?"));
                                    for i in 0..s.citizens() as usize {
                                        let c = s.citizen(i);
                                        println!("  {} eq={:.2}", s.citizen_code(i), c.eq);
                                    }
                                }
                            }
                            None => eprintln!("no world loaded"),
                        },
                        Command::AddFood { amount } => match &mut sim {
                            Some(s) => {
                                s.add_food(amount);
                                println!("+{} food -> {}", amount, s.food);
                            }
                            None => eprintln!("no world loaded"),
                        },
                        Command::SetCap { amount } => match &mut sim {
                            Some(s) => {
                                s.set_cap(amount);
                                println!("cap set to {}", s.population_cap);
                            }
                            None => eprintln!("no world loaded"),
                        },
                        Command::Unknown(w) => eprintln!("unknown command '{}' (try 'help')", w),
                    }
                    last = Instant::now();
                    leftover = Duration::ZERO;
                    prompt();
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => { quit = true; break; }
            }
        }
        if quit { break; }

        let now = Instant::now();
        match sim.as_mut() {
            Some(s) if !paused => {
                leftover += now - last;
                last = now;
                while leftover >= tick_len {
                    let (gained, died) = s.tick();
                    if gained > 0 {
                        println!("+{} -> alive={} food={}", gained, s.citizens(), s.food);
                    }
                    if died > 0 {
                       println!("-{} -> dead={} alive={}", died, s.dead_count(), s.citizens());
                    }
                    leftover -= tick_len;
                }
            }
            _ => {
                last = now;
                leftover = Duration::ZERO;
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }

    println!("bye");
}