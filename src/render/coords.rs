use macroquad::{
    prelude::Rect,
    window::{screen_height, screen_width},
};

use crate::logic::grid::{Grid, RoadId, RoadOrientation};

use super::grid::GridRenderer;

pub struct GridCoords {
    horizontal: Vec<HorizontalRoadCoords>,
    vertical: Vec<VerticalRoadCoords>,
}

impl GridCoords {
    pub fn new(grid: Grid) -> Self {
        let mut horizontal = Vec::new();
        for index in 0..Grid::horizontal_roads() {
            let coords = HorizontalRoadCoords {
                grid: grid.clone(),
                index,
            };
            horizontal.push(coords);
        }

        let vertical = Vec::new();

        Self {
            horizontal,
            vertical,
        }
    }
}

pub trait RoadCoords {
    const WIDTH: f32 = 60.0;
    const OUTLINE_WIDTH: f32 = 2.0;

    fn id(&self) -> RoadId;
    fn orientation(&self) -> RoadOrientation;

    fn x1(&self) -> f32;
    fn y1(&self) -> f32;
    fn x2(&self) -> f32 {
        self.x1() + self.w()
    }
    fn y2(&self) -> f32 {
        self.y1() + self.h()
    }

    fn w(&self) -> f32;
    fn h(&self) -> f32;

    fn rect(&self) -> Rect {
        Rect::new(self.x1(), self.y1(), self.w(), self.h())
    }
}

pub struct HorizontalRoadCoords {
    grid: Grid,
    index: usize,
}

pub struct VerticalRoadCoords {
    grid: Grid,
    index: usize,
}

impl RoadCoords for HorizontalRoadCoords {
    fn orientation(&self) -> RoadOrientation {
        RoadOrientation::Horizontal
    }
    fn id(&self) -> RoadId {
        RoadId {
            orientation: self.orientation(),
            index: self.index,
        }
    }

    fn x1(&self) -> f32 {
        GridRenderer::MARGIN
    }
    fn y1(&self) -> f32 {
        GridRenderer::MARGIN
            + self.index as f32
                * (Self::WIDTH + GridRenderer::space_between_roads(self.orientation()))
    }
    fn w(&self) -> f32 {
        screen_width() - GridRenderer::MARGIN * 2.
    }
    fn h(&self) -> f32 {
        Self::WIDTH
    }
}

impl RoadCoords for VerticalRoadCoords {
    fn orientation(&self) -> RoadOrientation {
        RoadOrientation::Vertical
    }
    fn id(&self) -> RoadId {
        RoadId {
            orientation: self.orientation(),
            index: self.index,
        }
    }

    fn y1(&self) -> f32 {
        GridRenderer::MARGIN
    }
    fn x1(&self) -> f32 {
        GridRenderer::MARGIN
            + self.index as f32
                * (Self::WIDTH + GridRenderer::space_between_roads(self.orientation()))
    }
    fn h(&self) -> f32 {
        screen_height() - GridRenderer::MARGIN * 2.
    }
    fn w(&self) -> f32 {
        Self::WIDTH
    }
}
