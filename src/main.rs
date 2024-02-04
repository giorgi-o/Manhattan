use logic::grid::Grid;

mod logic {
    pub mod grid;
    pub mod car;
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

// fn tick() {
//     clear_background(BLACK);

//     // draw_line(40.0, 40.0, 100.0, 200.0, 15.0, BLUE);
//     // draw_rectangle(screen_width() / 2.0 - 60.0, 100.0, 120.0, 60.0, GREEN);
//     draw_circle(screen_width() - 30.0, screen_height() - 30.0, 15.0, YELLOW);
//     // draw_text("HELLO", 20.0, 20.0, 20.0, DARKGRAY);
// }

