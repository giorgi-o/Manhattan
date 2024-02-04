use macroquad::prelude::*;

use crate::logic::{
    car::Car,
    grid::{Direction, Orientation},
};

use super::{
    grid::{GridRenderer, RoadRenderer},
    util::Lengths,
};

pub struct CarRenderer<'g> {
    car: &'g Car,
    grid_renderer: &'g GridRenderer<'g>,
    road_renderers: &'g Vec<RoadRenderer<'g>>,
    // position: &'g CarPosition,
}

impl<'g> CarRenderer<'g> {
    const ROAD_EDGE_MARGIN: f32 = 1.0;
    const BETWEEN_CARS_MARGIN: f32 = 1.0;

    // whether we drive on the left side of the road
    pub const ENGLAND_MODE: bool = true;

    // const COLOUR: Color = RED;

    pub fn new(
        car: &'g Car,
        grid_renderer: &'g GridRenderer<'g>,
        road_renderers: &'g Vec<RoadRenderer<'g>>,
        // position: &'g CarPosition,
    ) -> Self {
        Self {
            car,
            grid_renderer,
            road_renderers,
            // position,
        }
    }

    pub fn car_length() -> f32 {
        let section_length = RoadRenderer::section_lengths().min();
        let cars_per_section = RoadRenderer::cars_per_section().min();

        // section length = car count * car length + (car count - 1) * car margin
        // => car length = (section length - (car count - 1) * car margin) / car count

        (section_length - (cars_per_section - 1.0) * Self::BETWEEN_CARS_MARGIN) / cars_per_section
    }

    pub fn car_width() -> f32 {
        RoadRenderer::WIDTH / 2.0 - Self::ROAD_EDGE_MARGIN * 2.0
    }

    pub fn on_positive_side_of_road(&self) -> bool {
        // the "positive" side is the road lane furthest from 0, 0
        // 0, 0 is top left

        let direction = self.car.position.road_section.direction;

        let mut positive = direction == Direction::Down || direction == Direction::Left;
        if !Self::ENGLAND_MODE {
            positive = !positive;
        }

        positive
    }

    pub fn render(&self) {
        let road = self.road();
        let position = self.car.position;
        let orientation = road.orientation;

        let mut section_position = position.position_in_section;
        if !position.road_section.direction.towards_positive() {
            let max_section_position =
                RoadRenderer::cars_per_section().get(orientation) as usize - 1;

            section_position = max_section_position - section_position;
        }

        // coordinate of the middle of the two road lanes
        let road_middle = Lengths::from_vec2(road.rect.center()).get(orientation);

        // let lower_road_side; // most negative of the two
        // let upper_road_side;
        // let on_positive_side_of_road = self.on_positive_side_of_road();

        // if on_positive_side_of_road {
        //     lower_road_side = road_middle;
        //     upper_road_side = road_middle + RoadRenderer::WIDTH / 2.0;
        // } else {
        //     lower_road_side = road_middle - RoadRenderer::WIDTH / 2.0;
        //     upper_road_side = road_middle;
        // }

        // let car_width_lower; // the lowest coord of the car's long side
        // let car_width_upper;
        // let car_length_lower;
        // let car_length_upper; // the most positive coord of the car's short side
        // if the car is going horizontally,
        // car_width_* are the y coords
        // car_length_* are the x coords
        // because the car is longer than it is wide.
        // sorry, couldn't think of a better way to represent this

        // let section_lower_coord = RoadRenderer::WIDTH
        //     + (RoadRenderer::section_lengths() + RoadRenderer::WIDTH)
        //         * self.position.road_section.section_index as f32;
        let section_rect = road.section_rect(position.road_section.section_index);
        // let section_lower_coord = Lengths {
        //     v: section_rect.x,
        //     h: section_rect.y,
        // };

        // tmp: draw rectangle over current section
        // draw_rectangle(
        //     section_rect.x,
        //     section_rect.y,
        //     section_rect.w,
        //     section_rect.h,
        //     BLUE,
        // );
        // println!("drawing section: {:?}", position.road_section);

        // start with section rect as base
        let mut car_rect = section_rect;

        // adjust rect size
        if orientation == Orientation::Horizontal {
            car_rect.w = Self::car_length();
            car_rect.h = Self::car_width();
        } else {
            car_rect.h = Self::car_length();
            car_rect.w = Self::car_width();
        }

        // rect doesn't start at section start, it's somewhere along it
        let distance_between_cars = (RoadRenderer::section_lengths()
            - Self::car_length() * RoadRenderer::cars_per_section())
            / (RoadRenderer::cars_per_section() - 1.0);
        let distance_from_section_start =
            section_position as f32 * (Self::car_length() + distance_between_cars);

        if orientation == Orientation::Horizontal {
            car_rect.x += distance_from_section_start.h;
        } else {
            car_rect.y += distance_from_section_start.v;
        }

        // adjust side of the road
        if self.on_positive_side_of_road() {
            if orientation == Orientation::Horizontal {
                car_rect.y += RoadRenderer::WIDTH / 2.0;
            } else {
                car_rect.x += RoadRenderer::WIDTH / 2.0;
            }
        }

        // let rect = if orientation == Orientation::Horizontal {
        //     Rect::new(
        //         // section_lower_coord.get(orientation) + distance_from_section_start.get(orientation),
        //         // lower_road_side,
        //         section_rect.x + distance_from_section_start.h,
        //         section_rect.y,
        //         Self::car_length(),
        //         Self::car_width(),
        //     )
        // } else {
        //     Rect::new(
        //         // lower_road_side,
        //         // section_lower_coord.get(orientation) + distance_from_section_start.get(orientation),
        //         section_rect.x,
        //         section_rect.y + distance_from_section_start.v,
        //         Self::car_width(),
        //         Self::car_length(),
        //     )
        // };

        draw_rectangle(
            car_rect.x,
            car_rect.y,
            car_rect.w,
            car_rect.h,
            self.car.props.colour,
        );
    }

    fn road(&self) -> RoadRenderer<'g> {
        let road_section = self.car.position.road_section;

        // self.road_renderers
        //     .iter()
        //     .find(|r| {
        //         r.orientation == road_section.direction.orientation()
        //             && r.index == road_section.road_index
        //     })
        //     .unwrap()
        self.grid_renderer.road_at(road_section)
    }
}
