use std::time::{Duration, Instant};
use sim::{Sim, TICKS_PER_SECOND};

fn main() {
    let mut sim = Sim::new(12345);

    let tick_len = Duration::from_secs_f64(1.0 / TICKS_PER_SECOND);
    let mut last = Instant::now();
    let mut leftover = Duration::ZERO;

    loop {
        let now = Instant::now();
        leftover += now - last;
        last = now;

        while leftover >= tick_len {
            sim.tick();
            leftover -= tick_len;
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}