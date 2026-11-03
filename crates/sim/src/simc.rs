pub struct Sim {
    pub world_seed: u64,
    pub tick_count: u64,
}

impl Sim {
    pub fn new(world_seed: u64) -> Self {
        Sim {
            world_seed,
            tick_count: 0,
        }
    }

    pub fn tick(&mut self) {
        self.tick_count += 1;
    }
}