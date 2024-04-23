use macroquad::{
    color::*,
    math::Rect,
    shapes::{draw_rectangle, draw_rectangle_lines},
    text::draw_text,
};

use crate::logic::ev::ChargingStation;

use super::{grid::GridRenderer, util::RoadCoords};

pub struct ChargingStationRenderer;

impl ChargingStationRenderer {
    const DISTANCE_FROM_ROAD: f32 = 5.0;
    const SIDE_LENGTH: f32 = 15.0;

    pub fn render(grid: &GridRenderer, charging_station: &ChargingStation) {
        let position = charging_station.entrance;
        let road_coords = RoadCoords::new(position, grid);

        // store rects of all cars that this charging station can accomodate
        // ccar = charging car
        let mut ccar_rects = Vec::with_capacity(charging_station.capacity);
        for i in 0..charging_station.capacity {
            let (cx, cy) = road_coords
                .sidewalk_coords(Self::DISTANCE_FROM_ROAD + i as f32 * Self::SIDE_LENGTH);

            let hs = Self::SIDE_LENGTH / 2.0; // hs = half side
            let (x1, y1) = (cx - hs, cy - hs);
            let rect = Rect::new(x1, y1, Self::SIDE_LENGTH, Self::SIDE_LENGTH);
            ccar_rects.push(rect);
        }

        // get rect that surrounds them all
        let mut x1 = f32::INFINITY;
        let mut y1 = f32::INFINITY;
        let mut x2 = f32::NEG_INFINITY;
        let mut y2 = f32::NEG_INFINITY;
        for rect in &ccar_rects {
            x1 = x1.min(rect.left());
            y1 = y1.min(rect.top());
            x2 = x2.max(rect.right());
            y2 = y2.max(rect.bottom());
        }

        // draw charging station rect as green outline
        draw_rectangle_lines(x1, y1, x2 - x1, y2 - y1, 3.0, LIME);

        // draw charging cars
        for (car_id, rect) in charging_station.cars.iter().zip(ccar_rects.iter()) {
            let car = grid.grid.car(*car_id);

            draw_rectangle(rect.x, rect.y, rect.w, rect.h, car.props.colour);

            // write how many passengers the car has
            let center = rect.center();
            let text = format!("{}", car.passengers.len());
            let font_size = rect.w;
            draw_text(&text, center.x, center.y, font_size, BLACK);
        }
    }
}
