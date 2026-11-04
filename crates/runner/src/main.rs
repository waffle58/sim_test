use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use sim::{Sim, POPULATION_CAP, TICKS_PER_SECOND};

const SAVE_DIR: &str = "saves";
const DEFAULT_SEED: u64 = 12345;

enum Command {
    New { name: String, seed: u64 },
    Load { name: String },
    Save,
    Pause,
    Resume,
    Status,
    List,
    Help,
    Quit,
    Unknown(String),
}
fn derive_seed_from_name(name: &str) -> u64 {
    // FNV-1a 64-bit. Stable per name, so `new alpha` always gives the
    // same world; different names give different worlds.
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
            let seed = parts
                .next()
                .and_then(|s| s.parse().ok())
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
        "list" | "ls" => Command::List,
        "help" | "?" => Command::Help,
        "quit" | "exit" | "q" => Command::Quit,
        other => Command::Unknown(other.to_string()),
    }
}

fn save_path(name: &str) -> PathBuf {
    PathBuf::from(SAVE_DIR).join(name)   // was: .join(format!("{name}.save"))
}

fn validate_save_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("save name is empty".into());
    }
    if name.len() > 32 {
        return Err("save name is longer than 32 characters".into());
    }
    if name == "." || name == ".." {
        return Err("'.' and '..' are not valid save names".into());
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(format!(
            "save name '{}' contains invalid characters (use letters, digits, _ or -)",
            name
        ));
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
    println!("  new <name> [seed]   start a fresh world (default seed {})", DEFAULT_SEED);
    println!("  load <name>         read saves/<name>.save");
    println!("  save                write current world to saves/<name>.save");
    println!("  pause / resume      stop or start simulation time");
    println!("  status              print current world summary");
    println!("  list                print every citizen in the current world");
    println!("  help                this list");
    println!("  quit                exit");
}

fn main() {
    if let Err(e) = std::fs::create_dir_all(SAVE_DIR) {
        eprintln!("could not create {}: {e}", SAVE_DIR);
        return;
    }

    println!("waiting to run");
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
                            match validate_save_name(&name) {
                                Ok(()) => {
                                    let s = Sim::new(seed);
                                    println!("new '{}' (seed {}), pop {} (cap {})",
                                            name, seed, s.citizens(), POPULATION_CAP);
                                    sim = Some(s);
                                    current_name = Some(name);
                                    paused = false;
                                }
                                Err(e) => eprintln!("{}", e),
                            }
                        }
                        Command::Load { name } => {
                            match validate_save_name(&name) {
                                Ok(()) => {
                                    let path = save_path(&name);
                                    match Sim::load(path.to_str().unwrap()) {
                                        Ok(s) => {
                                            println!("loaded '{}', pop {}, t={}",
                                                    name, s.citizens(), s.tick_count);
                                            sim = Some(s);
                                            current_name = Some(name);
                                            paused = false;
                                        }
                                        Err(e) => eprintln!("load failed: {e}"),
                                    }
                                }
                                Err(e) => eprintln!("{}", e),
                            }
                        }
                        Command::Save => match (&sim, &current_name) {
                            (Some(s), Some(name)) => {
                                let path = save_path(name);
                                match s.save(path.to_str().unwrap()) {
                                    Ok(()) => println!("saved '{}' -> {}", name, path.display()),
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
                                "'{}' seed={} pop {} / {} t={} {}",
                                name, s.world_seed, s.citizens(), POPULATION_CAP, s.tick_count,
                                if paused { "(paused)" } else { "(running)" }
                            ),
                            _ => println!("no world loaded"),
                        },
                        Command::List => match &sim {
                            Some(s) => {
                                println!("{} citizens in '{}':",
                                        s.citizens(), current_name.as_deref().unwrap_or("?"));
                                for i in 0..s.citizens() as usize {
                                    let c = s.citizen(i);
                                    println!("  {} eq={:.2}", s.citizen_code(i), c.eq);
                                }
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
                    let gained = s.tick();
                    if gained > 0 {
                        println!("+{} -> pop {}", gained, s.citizens());
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