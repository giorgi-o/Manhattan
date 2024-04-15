use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use logic::grid::Grid;
use python::bridge::bridge::{initialise_python, start_python};
use render::render_main::GridLock;

mod logic {
    pub mod car;
    pub mod car_agent;
    pub mod grid;
    pub mod passenger;
    pub mod pathfinding;
    pub mod util;
    pub mod ev;
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
        pub mod err_handling;
        pub mod py_grid;
    }
}

fn main() {
    initialise_python();
    start_python();
}
