use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use super::grid::GridRenderer;
use crate::logic::{
    car::{CarProps, RandomCar},
    grid::Grid,
};

use macroquad::prelude::*;

static GAME: OnceLock<Mutex<Game>> = OnceLock::new();

pub fn start(grid: Grid) {
    // macroquad's main() can't take any arguments.
    // so we sneak the game in through the back.

    let game = Game { grid };
    let mutex = Mutex::new(game);
    let _ = GAME.set(mutex);

    main()
}

#[macroquad::main(window_conf)]
async fn main() {
    let game = GAME.get().unwrap();
    let mut game = game.lock().unwrap();

    let time_per_tick = Duration::from_secs_f32(1.0 / Game::TICKS_PER_SEC as f32);
    let mut last_tick = Instant::now() - Duration::from_secs_f32(999.9);

    loop {
        if last_tick.elapsed() >= time_per_tick {
            game.tick();
            last_tick = Instant::now();
        }

        game.render();
        next_frame().await;
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Manhattan".to_owned(),
        window_width: 1500,
        window_height: 1000,
        ..Default::default()
    }
}

pub struct Game {
    grid: Grid,
}

impl Game {
    const BACKGROUND_COLOUR: Color = WHITE;

    pub const TICKS_PER_SEC: usize = 20;

    fn new() -> Self {
        let npc_car = RandomCar {};
        let npc_car = CarProps::new(npc_car, 10);

        let mut grid = Grid::new();
        grid.add_car(npc_car);

        Self { grid }
    }

    fn tick(&mut self) {
        self.grid.tick();
    }

    pub fn render(&self) {
        clear_background(Self::BACKGROUND_COLOUR);

        let grid_renderer = GridRenderer::new(&self.grid);
        grid_renderer.render();
    }
}
