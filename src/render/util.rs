use std::ops::{Add, Div, Mul, Sub};

use crate::logic::grid::Orientation;

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
