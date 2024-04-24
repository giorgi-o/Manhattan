use macroquad::prelude::*;

use crate::logic::{
    car::{Car, CarPosition}, car_agent::AgentAction, util::{Direction, Orientation, RoadSection}
};

use super::{
    grid::{GridRenderer, RoadRenderer},
    util::Line,
};

pub struct CarRenderer<'g> {
    car: &'g Car,
    grid_renderer: &'g GridRenderer<'g>,
}

impl<'g> CarRenderer<'g> {
    pub const ROAD_EDGE_MARGIN: f32 = 1.0;
    pub const BETWEEN_CARS_MARGIN: f32 = 1.0;

    // whether we drive on the left side of the road
    pub const ENGLAND_MODE: bool = true; // this should really be somewhere else...

    // const COLOUR: Color = RED;
    const HEADLIGHT_COLOUR: Color = YELLOW;
    const PATH_COLOUR: Color = GREEN;

    pub fn new(car: &'g Car, grid_renderer: &'g GridRenderer<'g>) -> Self {
        Self { car, grid_renderer }
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

    fn headlights_margin(&self) -> f32 {
        Self::car_width() / 5.0
    }

    pub fn render(&self) {
        self.render_car();
        self.render_path();
        self.render_passenger_count();
    }

    pub fn render_car(&self) {
        // if in charging station, skip
        if self.car.position.is_at_charging_station() {
            return;
        }

        let rect = self.rect();
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

    fn rect(&self) -> Rect {
        Self::rect_from_position(self.car.position, &self.road())
    }

    pub fn rect_from_position(position: CarPosition, road: &RoadRenderer) -> Rect {
        let orientation = road.orientation;

        let mut section_position = position.position_in_section;
        if !position.road_section.direction.towards_positive() {
            let max_section_position =
                RoadRenderer::cars_per_section().get(orientation) as usize - 1;

            section_position = max_section_position - section_position;
        }

        let direction = position.road_section.direction;
        let section_rect =
            road.section_rect_on_side(position.road_section.section_index, direction);

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

        car_rect
    }

    fn render_path(&self) {
        // don't render npc paths
        if self.car.props.agent.is_npc() {
            return;
        }

        let agent = &self.car.props.agent;
        let Some(agent) = agent.as_path_agent() else {
            return; // agent doesn't work using paths
        };

        let Some(path) = agent.get_path() else {
            return; // no path to draw
        };

        let mut sections = path.sections.iter().peekable();
        
        let path_colour = match path.action {
            None => BLACK,
            Some(AgentAction::PickUp(_)) => YELLOW,
            Some(AgentAction::DropOff(_)) => BLUE,
            Some(AgentAction::ChargeBattery(_)) => GREEN,
            Some(AgentAction::HeadTowards(_)) => BROWN,
        };

        let mut start = PathLineBound::Car(self.car.position);
        // for path_section in sections {
        while let Some(path_section) = sections.next() {
            let end = match sections.peek() {
                Some(next_section) => {
                    PathLineBound::SectionsIntersection(((*path_section), **next_section))
                }
                None => PathLineBound::Car(path.destination),
            };

            // edge case: if we are on the last road section, both start
            // and end will be CarPositions. in that case, we only render the
            // line if we are not at the destination yet.
            if let PathLineBound::Car(car_pos) = start {
                if let PathLineBound::Car(dest_pos) = end {
                    if car_pos.position_in_section >= dest_pos.position_in_section {
                        continue;
                    }
                }
            }

            self.render_path_line(start, end, path_colour);

            start = end;
        }
    }

    fn render_path_line(&self, start: PathLineBound, end: PathLineBound, colour: Color) {
        let (x1, y1) = self.get_line_xy(start, true);
        let (x2, y2) = self.get_line_xy(end, false);

        let line = Line { x1, y1, x2, y2 };
        line.draw(colour);
    }

    fn road(&self) -> RoadRenderer<'g> {
        let road_section = self.car.position.road_section;
        self.grid_renderer.road_at(road_section)
    }

    fn get_line_xy(&self, line_bound: PathLineBound, start: bool) -> (f32, f32) {
        let x;
        let y;

        match line_bound {
            PathLineBound::Car(car_pos) => {
                let road = self.grid_renderer.road_at(car_pos.road_section);

                let car_rect = Self::rect_from_position(car_pos, &road);
                let line_through_car = Line::through_rect_middle(car_rect, road.orientation);

                let towards_positive = car_pos.road_section.direction.towards_positive();
                if towards_positive && start {
                    // return (line_through_car.x2, line_through_car.y2);
                    x = line_through_car.x2;
                    y = line_through_car.y2;
                } else {
                    // return (line_through_car.x1, line_through_car.y1);
                    x = line_through_car.x1;
                    y = line_through_car.y1;
                }
            }

            PathLineBound::SectionsIntersection((s1, s2)) => {
                let road1 = self.grid_renderer.road_at(s1);
                let road2 = self.grid_renderer.road_at(s2);

                let rect1 = road1.section_rect_on_side(s1.section_index, s1.direction);
                let rect2 = road2.section_rect_on_side(s2.section_index, s2.direction);

                let line1 = Line::through_rect_middle(rect1, road1.orientation);
                let line2 = Line::through_rect_middle(rect2, road2.orientation);

                let intersection = line1.intersection(line2);

                (x, y) = match intersection {
                    Some((x, y)) => (x, y),
                    None => {
                        // they are parallel, i.e. two sections in straight line

                        // note: if s1 is behind s2, this will cause the line
                        // to go all the way across both sections. which is fine
                        // since the next section will just re-draw the same line
                        (line2.x1, line2.y1)
                    }
                };
            }
        }

        (x, y)
    }

    fn render_passenger_count(&self) {
        // also renders battery%

        if self.car.props.agent.is_npc() {
            return;
        }

        let rect = self.rect();
        let font_size = rect.w.max(rect.h);
        let center = rect.center();

        let mut text = format!("{:.0}%", self.car.battery.get() * 100.);

        let riding_passengers = self
            .car
            .passengers
            .iter()
            .filter(|p| p.is_dropping_off())
            .count();
        if riding_passengers > 0 {
            text += format!(" {riding_passengers}P").as_str();
        }

        draw_text(&text, center.x, center.y, font_size, BLACK);
    }
}

#[derive(Clone, Copy, Debug)]
enum PathLineBound {
    // start or end
    Car(CarPosition),
    SectionsIntersection((RoadSection, RoadSection)),
}
