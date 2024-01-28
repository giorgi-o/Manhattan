use std::rc::Rc;

use super::grid::GridRenderer;
use crate::logic::{
    car::{Car, CarAgent, RandomCar},
    grid::{Grid, GridInner},
};

use macroquad::prelude::*;

pub fn start() {
    main();
}

#[macroquad::main("Manhattan")]
async fn main() {
    let game = Game::new();

    loop {
        game.tick();
        game.render();
        next_frame().await;
    }
}

struct Game {
    grid: Grid,
}

impl Game {
    const BACKGROUND_COLOUR: Color = WHITE;

    fn new() -> Self {
        let npc_car = RandomCar {};

        let grid = Grid::new();
        grid.add_car(npc_car);

        Self { grid }
    }

    fn tick(&self) {
        self.grid.tick();
    }

    pub fn render(&self) {
        clear_background(Self::BACKGROUND_COLOUR);

        let grid_renderer = GridRenderer::new(self.grid.clone());
        grid_renderer.render();
    }
}
