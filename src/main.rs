use std::path::PathBuf;

use logic::grid::Grid;
use python::bridge::bridge::{get_agent_decision, initialise_python};

mod logic {
    pub mod car;
    pub mod grid;
    pub mod passenger;
    pub mod pathfinding;
}

mod render {
    pub mod car;
    pub mod grid;
    pub mod passenger;
    pub mod render_main;
    pub mod util;
}

mod python {
    pub mod bridge {
        pub mod bridge;
        pub mod types;
    }
}

fn main() {
    initialise_python();

    let grid = Grid::new();
    render::render_main::start(grid);
}

// fn main() {
//     initialise_python();
//     get_agent_decision(vec![1, 2, 3]);
// }
