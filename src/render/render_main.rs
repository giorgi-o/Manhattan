use std::{
    borrow::{Borrow, BorrowMut},
    ops::{Deref, DerefMut},
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};

use super::grid::GridRenderer;
use crate::logic::grid::Grid;

use macroquad::prelude::*;

// bridge from the grid engine
#[derive(Clone)]
pub struct GridLock {
    mutex: Arc<Mutex<Grid>>,
}

impl GridLock {
    pub fn new(grid: Grid) -> Self {
        Self {
            mutex: Arc::new(Mutex::new(grid)),
        }
    }

    pub fn lock(&self) -> impl DerefMut<Target = Grid> + '_ {
        self.mutex.lock().unwrap()
    }
}

#[derive(Clone)]
struct GridRenderGlobalState {
    version: usize,
    current_grid: Option<GridLock>,
}

impl GridRenderGlobalState {
    const fn default() -> Self {
        Self {
            version: 0,
            current_grid: None,
        }
    }
}

static GRID_STATE: Mutex<GridRenderGlobalState> = Mutex::new(GridRenderGlobalState::default());

pub fn new_grid(grid_bridge: GridLock) {
    // macroquad's main() can't take any arguments.
    // so we sneak the game in through the back door.

    // note that this function should be able to be called
    // multiple times, but only call macroquad's main() once

    let mut grid_state = GRID_STATE.lock().unwrap();
    let is_first_time = grid_state.current_grid.is_none();

    grid_state.current_grid = Some(grid_bridge);
    grid_state.version += 1;
    drop(grid_state);

    if is_first_time {
        std::thread::spawn(main);
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    render_main().await;
}

async fn render_main() {
    // new iteration for every new grid environment
    loop {
        let grid_ref = {
            let grid_state = GRID_STATE.lock().unwrap();
            grid_state.clone()
        };

        // for every tick in the current grid
        loop {

            // check we have latest grid
            {
                let our_version = grid_ref.version;
                let latest_version = {
                    let grid_state = GRID_STATE.lock().unwrap();
                    grid_state.version
                };
                if our_version != latest_version {
                    break;
                }
            }

            {
                let grid = grid_ref.current_grid.as_ref().unwrap().lock();

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
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Manhattan".to_string(),
        window_width: 1500,
        window_height: 1000,
        ..Default::default()
    }
}
