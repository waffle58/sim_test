use std::time::{Duration, Instant};
use sim::{Sim, TICKS_PER_SECOND};

fn main() {

    let mut sim = Sim::new(12345);
    let tick_len = Duration::from_secs_f64(1.0 / TICKS_PER_SECOND);
    let mut last = Instant::now();
    let mut leftover = Duration::ZERO;
    let mut last_report = Instant::now();
    let mut ticks_since_report = 0u64;

    loop {
        let now = Instant::now();
            leftover += now - last;
            last = now;

            while leftover >= tick_len {
                sim.tick();
                ticks_since_report += 1;
                leftover -= tick_len;
            }

            if last_report.elapsed() >= Duration::from_secs(1) {
                println!("{} ticks/s, total {}", ticks_since_report, sim.tick_count);
                ticks_since_report = 0;
                last_report = Instant::now();
            }

        std::thread::sleep(Duration::from_millis(1));
        }  

}