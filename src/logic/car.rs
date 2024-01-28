use rand::seq::SliceRandom;

use super::grid::{RoadId, RoadOrientation};

#[derive(Hash, PartialEq, Eq, Clone)]
pub struct CarPosition {
    pub current_road: RoadId,
    pub prev_parallel_road: RoadId,
    pub next_parallel_road: RoadId,
}

impl CarPosition {
    pub fn orientation(&self) -> CarOrientation {
        let heading_bottom_right = self.prev_parallel_road.index < self.next_parallel_road.index;
        match self.current_road.orientation {
            RoadOrientation::Horizontal if heading_bottom_right => CarOrientation::Right,
            RoadOrientation::Horizontal => CarOrientation::Left,
            RoadOrientation::Vertical if heading_bottom_right => CarOrientation::Down,
            RoadOrientation::Vertical => CarOrientation::Up,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CarOrientation {
    Up,
    Down,
    Left,
    Right,
}

impl CarOrientation {
    pub fn apply_decision(self, decision: CarDecision) -> Self {
        if decision == CarDecision::GoStraight {
            return self;
        }

        let clockwise = [Self::Up, Self::Right, Self::Down, Self::Left];
        let current_index = clockwise
            .iter()
            .position(|i| *i == self)
            .expect("Enum value not in clockwise array");

        let offset = match decision {
            CarDecision::TurnLeft => -1,
            CarDecision::TurnRight => 1,
            CarDecision::GoStraight => unreachable!(),
        };
        let new_index = current_index as isize + offset;
        let new_index = new_index.rem_euclid(clockwise.len() as isize);

        clockwise[new_index as usize]
    }
}

impl From<CarOrientation> for RoadOrientation {
    fn from(value: CarOrientation) -> Self {
        match value {
            CarOrientation::Up | CarOrientation::Down => RoadOrientation::Horizontal,
            CarOrientation::Left | CarOrientation::Right => RoadOrientation::Vertical,
        }
    }
}

#[derive(Clone)]
pub enum CarState {
    NotSpawnedYet,
    Straight {
        position: CarPosition,
        progress: f32, // bteween 0 and 1
    },
    Turning {
        // can also be "turning" from and to the same road i.e. going straight
        from: CarPosition,
        to: CarPosition,
        progress: f32,
    },
}

pub struct Car {
    pub state: CarState,
    pub agent: Box<dyn CarAgent>,
}

impl Car {
    // speed unit: "progress" per second
    pub const STRAIGHT_SPEED: f32 = 0.3;
    pub const TURN_SPEED: f32 = 1.;

    pub fn new(agent: impl CarAgent + 'static) -> Self {
        Self {
            state: CarState::NotSpawnedYet,
            agent: Box::new(agent),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CarDecision {
    TurnLeft,
    GoStraight,
    TurnRight,
}

pub trait CarAgent {
    fn turn(&self, position: &CarPosition, options: &[CarDecision]) -> CarDecision;
}

pub struct RandomCar {}

impl CarAgent for RandomCar {
    fn turn(&self, _position: &CarPosition, options: &[CarDecision]) -> CarDecision {
        *options
            .choose(&mut rand::thread_rng())
            .expect("List of possible car decisions is empty")
    }
}
