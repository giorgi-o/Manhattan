


use python::bridge::bridge::{initialise_python, start_python};


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
    pub mod ev;
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
