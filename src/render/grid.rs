use macroquad::prelude::*;

use crate::logic::{
    grid::{Grid, LightState},
    util::{Direction, Orientation, RoadSection},
};

use super::{
    car::CarRenderer,
    ev::ChargingStationRenderer,
    passenger::PassengerRenderer,
    util::{Lengths, ToLengths},
};

pub struct GridRenderer<'g> {
    pub grid: &'g Grid,
    pub roads: Vec<RoadRenderer<'g>>,
}

impl<'g> GridRenderer<'g> {
    pub const MARGIN: f32 = 60.0;

    const BACKGROUND_COLOUR: Color = WHITE;

    pub fn new(grid: &'g Grid) -> Self {
        let roads = Self::roads(grid);
        Self { grid, roads }
    }

    pub fn render(&self) {
        clear_background(Self::BACKGROUND_COLOUR);

        self.render_roads();
        self.render_intersections();
        self.draw_grid_outline();

        let traffic_lights_renderer = TrafficLightsRenderer::new(self);
        traffic_lights_renderer.render_all();

        for car in self.grid.cars() {
            let car_renderer = CarRenderer::new(car, self);
            car_renderer.render();
        }

        for passenger in self.grid.waiting_passengers() {
            PassengerRenderer::render_waiting(self, passenger);
        }

        for charging_station in self.grid.charging_stations.values() {
            ChargingStationRenderer::render(self, charging_station);
        }
    }

    fn roads(grid: &'g Grid) -> Vec<RoadRenderer> {
        let mut roads = Vec::new();

        // horizontal
        for i in 0..Grid::HORIZONTAL_ROADS {
            let road = RoadRenderer::new(Orientation::Horizontal, i as isize, grid);
            roads.push(road);
        }

        // vertical
        for i in 0..Grid::VERTICAL_ROADS {
            let road = RoadRenderer::new(Orientation::Vertical, i as isize, grid);
            roads.push(road);
        }

        roads
    }

    pub fn road_at(&self, section: RoadSection) -> RoadRenderer<'g> {
        RoadRenderer::new(
            section.direction.orientation(),
            section.road_index,
            self.grid,
        )
    }

    fn render_roads(&self) {
        for road in &self.roads {
            road.render();
        }
    }

    fn render_intersections(&self) {
        for horizontal_road in self
            .roads
            .iter()
            .filter(|r| r.orientation == Orientation::Horizontal)
        {
            for vertical_road in self
                .roads
                .iter()
                .filter(|r| r.orientation == Orientation::Vertical)
            {
                let Some(intersection_rect) = horizontal_road.rect.intersect(vertical_road.rect)
                else {
                    continue;
                };

                draw_rectangle(
                    intersection_rect.x,
                    intersection_rect.y,
                    intersection_rect.w,
                    intersection_rect.h,
                    RoadRenderer::COLOUR,
                );
            }
        }
    }

    fn draw_grid_outline(&self) {
        draw_rectangle_lines(
            Self::MARGIN,
            Self::MARGIN,
            screen_width() - Self::MARGIN * 2.,
            screen_height() - Self::MARGIN * 2.,
            RoadRenderer::OUTLINE_WIDTH,
            RoadRenderer::OUTLINE_COLOUR,
        );
    }

    // isn't this just section_lengths() ?
    pub fn space_between_roads(orientation: Orientation) -> f32 {
        let street_count = match orientation {
            Orientation::Horizontal => Grid::HORIZONTAL_ROADS,
            Orientation::Vertical => Grid::VERTICAL_ROADS,
        };
        let canvas_size = (match orientation {
            Orientation::Horizontal => screen_height(),
            Orientation::Vertical => screen_width(),
        }) - Self::MARGIN * 2.0;
        let all_streets_combined_width = street_count as f32 * RoadRenderer::WIDTH;
        (canvas_size - all_streets_combined_width) / (street_count - 1) as f32
    }

    pub fn window_dimensions() -> Lengths {
        Lengths {
            h: screen_width(),
            v: screen_height(),
        }
    }

    pub fn grid_dimensions() -> Lengths {
        Self::window_dimensions() - Self::MARGIN * 2.0
    }
}

pub struct RoadRenderer<'g> {
    pub grid: &'g Grid,
    pub orientation: Orientation,
    pub index: isize,
    pub rect: Rect,
}

impl<'g> RoadRenderer<'g> {
    // todo comment out these 2 they belong on roadcoords now
    pub const WIDTH: f32 = 60.0;
    pub const OUTLINE_WIDTH: f32 = 2.0;

    pub const LANE_DIVIDER_LENGTH: f32 = 25.0;
    pub const LANE_DIVIDER_THICKNESS: f32 = 4.0;
    pub const LANE_DIVIDER_SPACING: f32 = 10.0;

    pub const COLOUR: Color = LIGHTGRAY;
    pub const OUTLINE_COLOUR: Color = BLACK;
    pub const LANE_DIVIDER_COLOUR: Color = WHITE;

    fn new<I: TryInto<isize>>(orientation: Orientation, index: I, grid: &'g Grid) -> Self {
        let Ok(index) = index.try_into() else {
            unreachable!() // index is too big for isize
        };
        let road_offset = GridRenderer::MARGIN
            + index as f32 * (RoadRenderer::WIDTH + GridRenderer::space_between_roads(orientation));

        let rect = match orientation {
            Orientation::Horizontal => Rect::new(
                GridRenderer::MARGIN,
                road_offset,
                screen_width() - GridRenderer::MARGIN * 2.,
                RoadRenderer::WIDTH,
            ),
            Orientation::Vertical => Rect::new(
                road_offset,
                GridRenderer::MARGIN,
                RoadRenderer::WIDTH,
                screen_height() - GridRenderer::MARGIN * 2.,
            ),
        };

        Self {
            grid,
            index,
            orientation,
            rect,
        }
    }

    fn lane_dividers(&self) -> Vec<Rect> {
        let mut rects = Vec::new();

        // for horizontal roads:
        // fixed_coord is y (it's the same for all stripes)
        // variable_coord is x (it's different for each stripe)
        let fixed_coord = match self.orientation {
            Orientation::Horizontal => self.rect.y,
            Orientation::Vertical => self.rect.x,
        } + RoadRenderer::WIDTH / 2.;

        let mut variable_coord = GridRenderer::MARGIN + Self::LANE_DIVIDER_SPACING;
        let max_variable_coord = match self.orientation {
            Orientation::Horizontal => screen_width(),
            Orientation::Vertical => screen_height(),
        } - GridRenderer::MARGIN;

        while variable_coord < max_variable_coord {
            let (x, y, w, h) = match self.orientation {
                Orientation::Horizontal => (
                    variable_coord,
                    fixed_coord,
                    Self::LANE_DIVIDER_LENGTH,
                    Self::LANE_DIVIDER_THICKNESS,
                ),
                Orientation::Vertical => (
                    fixed_coord,
                    variable_coord,
                    Self::LANE_DIVIDER_THICKNESS,
                    Self::LANE_DIVIDER_LENGTH,
                ),
            };

            let rect = Rect::new(x, y, w, h);
            rects.push(rect);

            variable_coord += Self::LANE_DIVIDER_LENGTH + Self::LANE_DIVIDER_SPACING;
        }

        rects
    }

    fn render(&self) {
        draw_rectangle(
            self.rect.x,
            self.rect.y,
            self.rect.w,
            self.rect.h,
            Self::COLOUR,
        );
        draw_rectangle_lines(
            self.rect.x,
            self.rect.y,
            self.rect.w,
            self.rect.h,
            RoadRenderer::OUTLINE_WIDTH,
            Self::OUTLINE_COLOUR,
        );

        for divider in self.lane_dividers() {
            draw_rectangle(
                divider.x,
                divider.y,
                divider.w,
                divider.h,
                Self::LANE_DIVIDER_COLOUR,
            );
        }
    }

    pub fn road_counts() -> Lengths {
        Lengths {
            h: Grid::HORIZONTAL_ROADS as f32,
            v: Grid::VERTICAL_ROADS as f32,
        }
    }

    pub fn cars_per_section() -> Lengths {
        Lengths {
            h: Grid::HORIZONTAL_SECTION_SLOTS as f32,
            v: Grid::VERTICAL_SECTION_SLOTS as f32,
        }
    }

    pub fn section_lengths() -> Lengths {
        // if we have 4 roads, there are 3 sections in between them
        // 3 * section length = grid length - 4 * road width
        let road_width = Self::WIDTH.lengths();
        let perpendicular_road_counts = Self::road_counts().inv();
        let section_counts = perpendicular_road_counts - 1.0;

        (GridRenderer::grid_dimensions() - road_width * perpendicular_road_counts) / section_counts
    }

    pub fn section_rect(&self, section_index: isize) -> Rect {
        let orientation = self.orientation;

        // get the rect of the entire road
        let mut rect = self.rect;
        // then edit it to fit the section only

        let section_start = GridRenderer::MARGIN
            + RoadRenderer::WIDTH
            + (Self::section_lengths().get(orientation) + RoadRenderer::WIDTH)
                * section_index as f32;

        if orientation == Orientation::Horizontal {
            rect.x = section_start;
            rect.w = Self::section_lengths().h;
        } else {
            rect.y = section_start;
            rect.h = Self::section_lengths().v;
        }

        rect
    }

    pub fn on_positive_side_of_road(direction: Direction) -> bool {
        // the "positive" side is the road lane furthest from 0, 0
        // 0, 0 is top left

        let mut positive = direction == Direction::Down || direction == Direction::Left;
        if !CarRenderer::ENGLAND_MODE {
            positive = !positive;
        }

        positive
    }

    pub fn section_rect_on_side(&self, section_index: isize, direction: Direction) -> Rect {
        let mut section_rect = self.section_rect(section_index);

        let positive_side = Self::on_positive_side_of_road(direction);
        match self.orientation {
            Orientation::Horizontal => {
                section_rect.h /= 2.0;
                if positive_side {
                    section_rect.y += section_rect.h;
                }
            }
            Orientation::Vertical => {
                section_rect.w /= 2.0;
                if positive_side {
                    section_rect.x += section_rect.w;
                }
            }
        }

        section_rect
    }
}

struct TrafficLightsRenderer<'g> {
    grid_renderer: &'g GridRenderer<'g>,
}

impl<'g> TrafficLightsRenderer<'g> {
    const SQUARE_SIZE: f32 = 15.0;
    const CIRCLE_RADIUS: f32 = 5.0;
    // const DISTANCE_FROM_ROAD: f32 = 10.0;
    const SIDE_MARGIN: f32 = 3.0;
    const BACK_MARGIN: f32 = 5.0;

    const SQUARE_COLOUR: Color = GRAY;
    const RED: Color = RED;
    const GREEN: Color = GREEN;

    pub fn new(grid_renderer: &'g GridRenderer) -> Self {
        Self { grid_renderer }
    }

    pub fn render_all(&self) {
        self.render_all_lights();
    }

    fn render_all_lights(&self) {
        // go through the empty spaces between the roads
        // and draw 4 traffic lights each time

        for x in -1..Grid::VERTICAL_ROADS as isize {
            for y in -1..Grid::HORIZONTAL_ROADS as isize {
                // the road section to the left of the blank space
                let right = RoadSection::get_raw(Direction::Up, x + 1, y);
                // println!("right: {:?}", right);

                let right_road_renderer = self.grid_renderer.road_at(right);
                let right_rect = right_road_renderer.section_rect(right.section_index);

                let top_right_corner = Vec2::new(right_rect.left(), right_rect.top());
                let section_lengths = RoadRenderer::section_lengths();
                let rect = Rect::new(
                    top_right_corner.x - section_lengths.h,
                    top_right_corner.y,
                    section_lengths.h,
                    section_lengths.v,
                );

                // "inner rect" whose 4 corners are the centers of the 4
                // traffic light squares
                // rect.x += offset;
                // rect.y += offset;
                // rect.w -= offset * 2.0;
                // rect.h -= offset * 2.0;

                let mut back_offset = Self::BACK_MARGIN + Self::SQUARE_SIZE / 2.0;
                let mut side_offset = Self::SIDE_MARGIN + Self::SQUARE_SIZE / 2.0;
                if !CarRenderer::ENGLAND_MODE {
                    (side_offset, back_offset) = (back_offset, side_offset);
                }

                // let topleft_direction = match CarRenderer::ENGLAND_MODE {
                //     true => Direction::Right,
                //     false => Direction::Down,
                // };

                // function to invert direction if not in england
                let i = |d: Direction| match CarRenderer::ENGLAND_MODE {
                    true => d,
                    false => d.clockwise().clockwise(),
                };

                // calculate the section coordinates of the sections around us
                // let left = RoadSection::get(i(Direction::Down), x, y);
                // let top = RoadSection::get(i(Direction::Left), y, x);
                // let right = RoadSection::get(i(Direction::Up), x + 1, y);
                // let bottom = RoadSection::get(i(Direction::Right), y + 1, x);
                // let left = (x >= 0).then(|| RoadSection {
                //     direction: i(Direction::Down),
                //     road_index: x as usize,
                //     section_index: y as usize,
                // });
                let left = (x >= 0).then(|| RoadSection::get_raw(i(Direction::Down), x, y));
                // let top = (y >= 0).then(|| RoadSection {
                //     direction: i(Direction::Left),
                //     road_index: y as usize,
                //     section_index: x as usize,
                // });
                let top = (y >= 0).then(|| RoadSection::get_raw(i(Direction::Left), y, x));
                // let right = RoadSection {
                //     direction: i(Direction::Up),
                //     road_index: (x + 1) as usize,
                //     section_index: y as usize,
                // };
                let right = RoadSection::get_raw(i(Direction::Up), x + 1, y);
                // let bottom = RoadSection {
                //     direction: i(Direction::Right),
                //     road_index: (y + 1) as usize,
                //     section_index: x as usize,
                // };
                let bottom = RoadSection::get_raw(i(Direction::Right), y + 1, x);

                // now, we draw all 4 traffic lights
                if let Some(top) = top {
                    if let Some(left) = left {
                        // top-left
                        self.render_light(
                            match CarRenderer::ENGLAND_MODE {
                                true => top,
                                false => left,
                            },
                            rect.left() + back_offset,
                            rect.top() + side_offset,
                        );
                    }
                    // top-right
                    self.render_light(
                        match CarRenderer::ENGLAND_MODE {
                            true => right,
                            false => top,
                        },
                        rect.right() - side_offset,
                        rect.top() + back_offset,
                    );
                }
                // bottom-right
                self.render_light(
                    match CarRenderer::ENGLAND_MODE {
                        true => bottom,
                        false => right,
                    },
                    rect.right() - back_offset,
                    rect.bottom() - side_offset,
                );
                if let Some(left) = left {
                    // bottom-left
                    self.render_light(
                        match CarRenderer::ENGLAND_MODE {
                            true => left,
                            false => bottom,
                        },
                        rect.left() + side_offset,
                        rect.bottom() - back_offset,
                    );
                }
            }
        }
    }

    fn render_light(&self, section: RoadSection, cx: f32, cy: f32) {
        if section.valid().is_err() {
            return;
        }

        let light = self.grid_renderer.grid.traffic_light_at(&section);

        // square top-left
        let x = cx - Self::SQUARE_SIZE / 2.0;
        let y = cy - Self::SQUARE_SIZE / 2.0;

        // red/green circle center
        let s = Self::SQUARE_SIZE;
        let s_2 = s / 2.0;
        let traffic_light_direction = section.direction.inverted();
        let (cx, cy) = match traffic_light_direction {
            Direction::Left => (cx - s_2, cy),
            Direction::Right => (cx + s_2, cy),
            Direction::Up => (cx, cy - s_2),
            Direction::Down => (cx, cy + s_2),
        };
        let colour = match light.state {
            LightState::Green => Self::GREEN,
            LightState::Red => Self::RED,
        };
        draw_circle(cx, cy, Self::CIRCLE_RADIUS, colour);

        // draw square
        draw_rectangle(x, y, s, s, Self::SQUARE_COLOUR);
    }
}
