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
    const HEADLIGHT_COLOUR: Color = YELLOW;

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

    fn headlights_margin(&self) -> f32 {
        Self::car_width() / 5.0
    }

    pub fn render(&self) {
        let rect = self.car_rect();
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, self.car.props.colour);

        // draw headlights
        let margin = self.headlights_margin();

        let direction = self.car.position.road_section.direction;
        let (x1, y1, x2, y2) = match direction {
            Direction::Up => (
                rect.left() + margin,
                rect.top() + margin,
                rect.right() - margin,
                rect.top() + margin,
            ),
            Direction::Down => (
                rect.left() + margin,
                rect.bottom() - margin,
                rect.right() - margin,
                rect.bottom() - margin,
            ),
            Direction::Left => (
                rect.left() + margin,
                rect.top() + margin,
                rect.left() + margin,
                rect.bottom() - margin,
            ),
            Direction::Right => (
                rect.right() - margin,
                rect.top() + margin,
                rect.right() - margin,
                rect.bottom() - margin,
            ),
        };

        let radius = margin / 2.0;
        draw_circle(x1, y1, radius, Self::HEADLIGHT_COLOUR);
        draw_circle(x2, y2, radius, Self::HEADLIGHT_COLOUR);
    }

    fn car_rect(&self) -> Rect {
        let road = self.road();
        let position = self.car.position;
        let orientation = road.orientation;

        let mut section_position = position.position_in_section;
        if !position.road_section.direction.towards_positive() {
            let max_section_position =
                RoadRenderer::cars_per_section().get(orientation) as usize - 1;

            section_position = max_section_position - section_position;
        }

        let section_rect = road.section_rect(position.road_section.section_index);

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

        car_rect
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
