use std::ops::{Add, Div, Mul, Sub};

use macroquad::prelude::*;

use crate::{
    logic::{
        car::CarPosition,
        util::{Direction, Orientation},
    },
    render::grid::RoadRenderer,
};

use super::{car::CarRenderer, grid::GridRenderer};

// util struct for abstracting over whether we are in horizontal or vertical
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Lengths {
    pub h: f32, // horizontal
    pub v: f32, // vertical
}

impl Lengths {
    pub fn from_vec2(vec2: macroquad::math::Vec2) -> Self {
        Self {
            h: vec2.y,
            v: vec2.x,
        }
    }

    pub fn get(self, orientation: Orientation) -> f32 {
        match orientation {
            Orientation::Horizontal => self.h,
            Orientation::Vertical => self.v,
        }
    }

    pub fn inv(self) -> Self {
        Self {
            h: self.v,
            v: self.h,
        }
    }

    pub fn min(self) -> f32 {
        self.h.min(self.v)
    }
}

impl Add for Lengths {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            h: self.h + rhs.h,
            v: self.v + rhs.v,
        }
    }
}

impl Add<f32> for Lengths {
    type Output = Self;

    fn add(self, rhs: f32) -> Self::Output {
        Self {
            h: self.h + rhs,
            v: self.v + rhs,
        }
    }
}

impl Add<Lengths> for f32 {
    type Output = Lengths;

    fn add(self, rhs: Lengths) -> Self::Output {
        Lengths {
            h: self + rhs.h,
            v: self + rhs.v,
        }
    }
}

impl Sub for Lengths {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            h: self.h - rhs.h,
            v: self.v - rhs.v,
        }
    }
}

impl Sub<f32> for Lengths {
    type Output = Self;

    fn sub(self, rhs: f32) -> Self::Output {
        Self {
            h: self.h - rhs,
            v: self.v - rhs,
        }
    }
}

impl Sub<Lengths> for f32 {
    type Output = Lengths;

    fn sub(self, rhs: Lengths) -> Self::Output {
        Lengths {
            h: self - rhs.h,
            v: self - rhs.v,
        }
    }
}

impl Mul for Lengths {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            h: self.h * rhs.h,
            v: self.v * rhs.v,
        }
    }
}

impl Mul<f32> for Lengths {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            h: self.h * rhs,
            v: self.v * rhs,
        }
    }
}

impl Mul<Lengths> for f32 {
    type Output = Lengths;

    fn mul(self, rhs: Lengths) -> Self::Output {
        Lengths {
            h: self * rhs.h,
            v: self * rhs.v,
        }
    }
}

impl Div for Lengths {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self {
            h: self.h / rhs.h,
            v: self.v / rhs.v,
        }
    }
}

impl Div<f32> for Lengths {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self {
            h: self.h / rhs,
            v: self.v / rhs,
        }
    }
}

impl Div<Lengths> for f32 {
    type Output = Lengths;

    fn div(self, rhs: Lengths) -> Self::Output {
        Lengths {
            h: self / rhs.h,
            v: self / rhs.v,
        }
    }
}

pub trait ToLengths {
    fn lengths(self) -> Lengths;
}

impl ToLengths for f32 {
    fn lengths(self) -> Lengths {
        Lengths { h: self, v: self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl Line {
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    pub fn draw(self, colour: Color) {
        draw_line(self.x1, self.y1, self.x2, self.y2, 1.0, colour);
    }

    pub fn flip(&mut self) {
        std::mem::swap(&mut self.x1, &mut self.x2);
        std::mem::swap(&mut self.y1, &mut self.y2);
    }

    pub fn through_rect_middle(rect: Rect, orientation: Orientation) -> Self {
        match orientation {
            Orientation::Horizontal => {
                let y = rect.top() + rect.h / 2.0;
                Self {
                    x1: rect.left(),
                    y1: y,
                    x2: rect.right(),
                    y2: y,
                }
            }
            Orientation::Vertical => {
                let x = rect.left() + rect.w / 2.0;
                Self {
                    x1: x,
                    y1: rect.top(),
                    x2: x,
                    y2: rect.bottom(),
                }
            }
        }
    }

    pub fn intersection(self, other: Line) -> Option<(f32, f32)> {
        // https://en.wikipedia.org/wiki/Line%E2%80%93line_intersection#Given_two_points_on_each_line
        let (x1, y1, x2, y2) = (self.x1, self.y1, self.x2, self.y2);
        let (x3, y3, x4, y4) = (other.x1, other.y1, other.x2, other.y2);

        let x_denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);
        let y_denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);
        if x_denom == 0.0 || y_denom == 0.0 {
            return None; // parallel
        }

        let x = ((x1 * y2 - y1 * x2) * (x3 - x4) - (x1 - x2) * (x3 * y4 - y3 * x4)) / x_denom;
        let y = ((x1 * y2 - y1 * x2) * (y3 - y4) - (y1 - y2) * (x3 * y4 - y3 * x4)) / y_denom;

        Some((x, y))
    }
}

pub struct RoadCoords {
    position: CarPosition,
    x: f32,
    y: f32,
    sidewalk_direction: Direction,
}

impl RoadCoords {
    pub fn new(position: CarPosition, grid: &GridRenderer) -> Self {
        // get the rectangle of the car
        let road = grid.road_at(position.road_section);
        let car_rect = CarRenderer::rect_from_position(position, &road);

        // get the center of the rectangle
        let x = car_rect.left() + car_rect.w / 2.0;
        let y = car_rect.top() + car_rect.h / 2.0;

        // get the direction towards the sidewalk
        let road_direction = position.road_section.direction;
        let sidewalk_direction = match CarRenderer::ENGLAND_MODE {
            true => road_direction.counterclockwise(),
            false => road_direction.clockwise(),
        };

        Self {
            position,
            x,
            y,
            sidewalk_direction,
        }
    }

    pub fn offset_coords(&self, offset: f32) -> (f32, f32) {
        match self.sidewalk_direction {
            Direction::Up => (self.x, self.y - offset),
            Direction::Down => (self.x, self.y + offset),
            Direction::Left => (self.x - offset, self.y),
            Direction::Right => (self.x + offset, self.y),
        }
    }

    pub fn sidewalk_coords(&self, offset: f32) -> (f32, f32) {
        self.offset_coords(RoadRenderer::WIDTH / 2.0 + offset)
    }
}
