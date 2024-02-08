use logic::grid::Grid;

mod logic {
    pub mod grid;
    pub mod car;
    pub mod pathfinding;
    pub mod passenger;
}

mod render {
    pub mod render_main;
    pub mod util;
    pub mod grid;
    pub mod car;
    pub mod coords;
}

fn main() {
    let grid = Grid::new();
    render::render_main::start(grid);
}
