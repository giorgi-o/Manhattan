use std::{
    borrow::Borrow,
    ops::{Deref, DerefMut},
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};

use super::grid::GridRenderer;
use crate::logic::grid::Grid;

use macroquad::prelude::*;

// bridge from the grid engine
#[derive(Clone)]
pub struct GridRef {
    pub mutex: Arc<Mutex<Grid>>,
}

impl GridRef {
    pub fn lock(&self) -> impl DerefMut<Target = Grid> + '_ {
        self.mutex.lock().unwrap()
    }
}

static mut GRID_REF: OnceLock<GridRef> = OnceLock::new();

pub fn start(grid_bridge: GridRef) {
    // macroquad's main() can't take any arguments.
    // so we sneak the game in through the back door.

    // let game = Game { grid };
    // let mutex = Mutex::new(game);
    unsafe {
        let _ = GRID_REF.set(grid_bridge);
    }

    std::thread::spawn(main);
}

#[macroquad::main(window_conf)]
async fn main() {
    let grid_ref = unsafe { GRID_REF.take().unwrap() };

    // let time_per_tick = Duration::from_secs_f32(1.0 / Game::TICKS_PER_SEC as f32);
    // let mut last_tick = Instant::now() - Duration::from_secs_f32(999.9);

    loop {
        {
            let grid = grid_ref.lock();

            // if grid.done() {
            //     return;
            // }

            let renderer = GridRenderer::new(&grid);
            renderer.render();
        }

        // if last_tick.elapsed() >= time_per_tick {
        //     game.tick();
        //     last_tick = Instant::now();
        // }

        // game.render();

        next_frame().await;
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Manhattan".to_string(),
        window_width: 1500,
        window_height: 1000,
        ..Default::default()
    }
}
