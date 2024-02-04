/*
use macroquad::{
    prelude::Rect,
    window::{screen_height, screen_width},
};

use crate::logic::grid::Grid;

use super::grid::{GridRenderer, RoadOrientation};

pub struct GridCoords<'g> {
    grid: &'g Grid,
    horizontal: Vec<HorizontalRoadCoords<'g>>,
    vertical: Vec<VerticalRoadCoords<'g>>,
}

impl<'g> GridCoords<'g> {
    pub fn new(grid: &'g Grid) -> Self {
        let mut horizontal = Vec::new();
        for index in 0..Grid::HORIZONTAL_ROADS {
            let coords = HorizontalRoadCoords { grid, index };
            horizontal.push(coords);
        }

        let vertical = Vec::new();

        Self {
            grid,
            horizontal,
            vertical,
        }
    }
}

pub trait RoadCoords {
    const WIDTH: f32 = 60.0;
    const OUTLINE_WIDTH: f32 = 2.0;

    fn index(&self) -> usize;
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

pub struct HorizontalRoadCoords<'g> {
    grid: &'g Grid,
    index: usize,
}

pub struct VerticalRoadCoords<'g> {
    grid: &'g Grid,
    index: usize,
}

impl RoadCoords for HorizontalRoadCoords<'_> {
    fn orientation(&self) -> RoadOrientation {
        RoadOrientation::Horizontal
    }
    fn index(&self) -> usize {
        self.index
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

impl RoadCoords for VerticalRoadCoords<'_> {
    fn orientation(&self) -> RoadOrientation {
        RoadOrientation::Vertical
    }
    fn index(&self) -> usize {
        self.index
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
*/