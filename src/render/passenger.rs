use macroquad::prelude::*;

use crate::logic::{grid::Orientation, passenger::Passenger};

use super::{
    car::CarRenderer,
    grid::{GridRenderer, RoadRenderer},
    util::Line,
};

pub struct PassengerRenderer {
    // pub struct PassengerRenderer<'g> {
    // grid: &'g Grid,
    // passenger: &'g Passenger,
}

impl PassengerRenderer {
    const DISTANCE_FROM_ROAD: f32 = 10.0;
    const RADIUS: f32 = 5.0;

    pub fn render_waiting(grid: &GridRenderer, passenger: &Passenger) {
        let position = passenger.start;
        let road = &grid.road_at(position.road_section);
        let car_rect = CarRenderer::rect_from_position(position, road);

        let line_through_car = Line::through_rect_middle(car_rect, road.orientation.other());
        let offset = CarRenderer::ROAD_EDGE_MARGIN + Self::DISTANCE_FROM_ROAD + Self::RADIUS;
        let positive_side_of_road =
            RoadRenderer::on_positive_side_of_road(position.road_section.direction);

        let mut cx;
        let mut cy;
        if positive_side_of_road {
            (cx, cy) = (line_through_car.x2, line_through_car.y2);
            match road.orientation {
                Orientation::Horizontal => {
                    cx += offset;
                }
                Orientation::Vertical => {
                    cy += offset;
                }
            }
        } else {
            (cx, cy) = (line_through_car.x1, line_through_car.y1);
            match road.orientation {
                Orientation::Horizontal => {
                    cx -= offset;
                }
                Orientation::Vertical => {
                    cy -= offset;
                }
            }
        };

        draw_circle(cx, cy, Self::RADIUS, passenger.colour);
    }
}
