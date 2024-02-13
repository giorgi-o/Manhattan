use std::path::PathBuf;

use pyo3::prelude::*;

use logic::grid::Grid;

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

fn main() {
    let grid = Grid::new();
    render::render_main::start(grid);
}

// fn main() {
//     Python::with_gil(|py| {
//         // add ./python/src to sys.path
//         let cwd = std::env::current_dir().unwrap();
//         let src_dir = cwd.join("python").join("src");

//         let sys = py.import("sys").unwrap();
//         let path = sys.getattr("path").unwrap();
//         path.call_method("append", (src_dir,), None).unwrap();

//         let main = py.import("main").unwrap();
//         main.call_method("hello_world", (), None).unwrap();
//     });
// }
