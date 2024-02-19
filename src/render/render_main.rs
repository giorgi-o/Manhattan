use std::{
    borrow::Borrow,
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};

use super::grid::GridRenderer;
use crate::logic::grid::Grid;

use macroquad::prelude::*;

// bridge from the grid engine
#[derive(Clone)]
pub struct GridBridge {
    pub mutex: Arc<Mutex<Grid>>,
}

static mut GAME: OnceLock<GridBridge> = OnceLock::new();

pub fn start(grid_bridge: GridBridge) {
    // macroquad's main() can't take any arguments.
    // so we sneak the game in through the back door.

    // let game = Game { grid };
    // let mutex = Mutex::new(game);
    unsafe {
        let _ = GAME.set(grid_bridge);
    }

    std::thread::spawn(main);
}

#[macroquad::main(window_conf)]
async fn main() {
    println!("starting render_main");

    let grid_bridge = unsafe { GAME.take().unwrap() };
    let mutex = grid_bridge.mutex.as_ref();

    // let time_per_tick = Duration::from_secs_f32(1.0 / Game::TICKS_PER_SEC as f32);
    // let mut last_tick = Instant::now() - Duration::from_secs_f32(999.9);

    loop {
        {
            let grid = mutex.lock().unwrap();

            if grid.done() {
                return;
            }

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
        window_title: "Manhattan".to_owned(),
        window_width: 1500,
        window_height: 1000,
        ..Default::default()
    }
}
