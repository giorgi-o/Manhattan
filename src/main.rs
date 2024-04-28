use logic::{car::CarPosition, pathfinding::Path, util::Direction};
use python::bridge::bridge::{initialise_python, start_python};

mod logic {
    pub mod car;
    pub mod car_agent;
    pub mod ev;
    pub mod grid;
    pub mod passenger;
    pub mod pathfinding;
    pub mod util;
    pub mod grid_util;
}

mod render {
    pub mod car;
    pub mod ev;
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
    std::env::set_var("RUST_BACKTRACE", "1");

    initialise_python();
    start_python();
}
