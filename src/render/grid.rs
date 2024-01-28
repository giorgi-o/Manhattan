use macroquad::prelude::*;

use crate::logic::{
    car::{Car, CarState},
    grid::{Grid, GridInner, RoadId, RoadOrientation},
};

pub struct GridRenderer {
    grid: Grid,
}

impl GridRenderer {
    pub const MARGIN: f32 = 40.0;

    pub fn new(grid: Grid) -> Self {
        Self { grid }
    }

    pub fn render(&self) {
        let roads = self.roads();
        self.render_roads(&roads);
        self.render_intersections(&roads);
        self.draw_grid_outline();
    }

    fn roads(&self) -> Vec<RoadRenderer> {
        let mut roads = Vec::new();

        // horizontal
        for i in 0..Grid::horizontal_roads() {
            let road = RoadRenderer::new(RoadOrientation::Horizontal, i, self.grid.clone());
            roads.push(road);
        }

        // vertical
        for i in 0..Grid::vertical_roads() {
            let road = RoadRenderer::new(RoadOrientation::Vertical, i, self.grid.clone());
            roads.push(road);
        }

        roads
    }

    fn render_roads(&self, roads: &[RoadRenderer]) {
        for road in roads {
            road.render();
        }
    }

    fn render_intersections(&self, roads: &[RoadRenderer]) {
        for horizontal_road in roads
            .iter()
            .filter(|r| r.id.orientation == RoadOrientation::Horizontal)
        {
            for vertical_road in roads
                .iter()
                .filter(|r| r.id.orientation == RoadOrientation::Vertical)
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

    pub fn space_between_roads(orientation: RoadOrientation) -> f32 {
        let street_count = match orientation {
            RoadOrientation::Horizontal => Grid::horizontal_roads(),
            RoadOrientation::Vertical => Grid::vertical_roads(),
        };
        let canvas_size = (match orientation {
            RoadOrientation::Horizontal => screen_height(),
            RoadOrientation::Vertical => screen_width(),
        }) - Self::MARGIN * 2.0;
        let all_streets_combined_width = street_count as f32 * RoadRenderer::WIDTH;
        (canvas_size - all_streets_combined_width) / (street_count - 1) as f32
    }
}

struct RoadRenderer {
    id: RoadId,
    rect: Rect,
    grid: Grid,
}

impl RoadRenderer {
    // todo comment out these 2 they belong on roadcoords now
    const WIDTH: f32 = 60.0;
    const OUTLINE_WIDTH: f32 = 2.0;

    const LANE_DIVIDER_LENGTH: f32 = 25.0;
    const LANE_DIVIDER_THICKNESS: f32 = 4.0;
    const LANE_DIVIDER_SPACING: f32 = 10.0;

    const COLOUR: Color = LIGHTGRAY;
    const OUTLINE_COLOUR: Color = BLACK;
    const LANE_DIVIDER_COLOUR: Color = WHITE;

    fn new(orientation: RoadOrientation, index: usize, grid: Grid) -> Self {
        let road_offset = GridRenderer::MARGIN
            + index as f32 * (RoadRenderer::WIDTH + GridRenderer::space_between_roads(orientation));

        let rect = match orientation {
            RoadOrientation::Horizontal => Rect::new(
                GridRenderer::MARGIN,
                road_offset,
                screen_width() - GridRenderer::MARGIN * 2.,
                RoadRenderer::WIDTH,
            ),
            RoadOrientation::Vertical => Rect::new(
                road_offset,
                GridRenderer::MARGIN,
                RoadRenderer::WIDTH,
                screen_height() - GridRenderer::MARGIN * 2.,
            ),
        };

        let road_id = RoadId::new(index, orientation);
        Self {
            id: road_id,
            rect,
            grid,
        }
    }

    fn lane_dividers(&self) -> Vec<Rect> {
        let mut rects = Vec::new();

        // for horizontal roads:
        // fixed_coord is y (it's the same for all stripes)
        // variable_coord is x (it's different for each stripe)
        let fixed_coord = match self.id.orientation {
            RoadOrientation::Horizontal => self.rect.y,
            RoadOrientation::Vertical => self.rect.x,
        } + RoadRenderer::WIDTH / 2.;

        let mut variable_coord = GridRenderer::MARGIN + Self::LANE_DIVIDER_SPACING;
        let max_variable_coord = match self.id.orientation {
            RoadOrientation::Horizontal => screen_width(),
            RoadOrientation::Vertical => screen_height(),
        } - GridRenderer::MARGIN;

        while variable_coord < max_variable_coord {
            let (x, y, w, h) = match self.id.orientation {
                RoadOrientation::Horizontal => (
                    variable_coord,
                    fixed_coord,
                    Self::LANE_DIVIDER_LENGTH,
                    Self::LANE_DIVIDER_THICKNESS,
                ),
                RoadOrientation::Vertical => (
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

    // cars should only be the cars going straight on this road
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
}
