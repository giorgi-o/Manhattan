use macroquad::color::Color;
use rand::{seq::SliceRandom, Rng};

use super::{
    grid::{Grid, Orientation, RoadSection},
    pathfinding::Path,
};

use macroquad::color::*;

// use super::grid::{RoadId, RoadOrientation};

// #[derive(Hash, PartialEq, Eq, Clone)]
// pub struct CarPosition {
//     pub current_road: RoadId,
//     pub prev_parallel_road: RoadId,
//     pub next_parallel_road: RoadId,
// }

// impl CarPosition {
//     pub fn orientation(&self) -> CarOrientation {
//         let heading_bottom_right = self.prev_parallel_road.index < self.next_parallel_road.index;
//         match self.current_road.orientation {
//             RoadOrientation::Horizontal if heading_bottom_right => CarOrientation::Right,
//             RoadOrientation::Horizontal => CarOrientation::Left,
//             RoadOrientation::Vertical if heading_bottom_right => CarOrientation::Down,
//             RoadOrientation::Vertical => CarOrientation::Up,
//         }
//     }
// }

// #[derive(Clone, Copy, PartialEq, Eq)]
// pub enum CarOrientation {
//     Up,
//     Down,
//     Left,
//     Right,
// }

// impl CarOrientation {
//     pub fn apply_decision(self, decision: CarDecision) -> Self {
//         if decision == CarDecision::GoStraight {
//             return self;
//         }

//         let clockwise = [Self::Up, Self::Right, Self::Down, Self::Left];
//         let current_index = clockwise
//             .iter()
//             .position(|i| *i == self)
//             .expect("Enum value not in clockwise array");

//         let offset = match decision {
//             CarDecision::TurnLeft => -1,
//             CarDecision::TurnRight => 1,
//             CarDecision::GoStraight => unreachable!(),
//         };
//         let new_index = current_index as isize + offset;
//         let new_index = new_index.rem_euclid(clockwise.len() as isize);

//         clockwise[new_index as usize]
//     }
// }

// impl From<CarOrientation> for RoadOrientation {
//     fn from(value: CarOrientation) -> Self {
//         match value {
//             CarOrientation::Up | CarOrientation::Down => RoadOrientation::Horizontal,
//             CarOrientation::Left | CarOrientation::Right => RoadOrientation::Vertical,
//         }
//     }
// }

// #[derive(Clone)]
// pub enum CarState {
//     NotSpawnedYet,
//     Straight {
//         position: CarPosition,
//         progress: f32, // bteween 0 and 1
//     },
//     Turning {
//         // can also be "turning" from and to the same road i.e. going straight
//         from: CarPosition,
//         to: CarPosition,
//         progress: f32,
//     },
// }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CarPosition {
    pub road_section: RoadSection,
    pub position_in_section: usize, // higher = further along
}

pub enum NextCarPosition {
    OnlyStraight(CarPosition),
    MustChoose(Vec<CarDecision>),
}

impl CarPosition {
    pub fn random(mut rng: impl Rng) -> Self {
        let road_section = RoadSection::random(&mut rng);

        Self {
            position_in_section: rng
                .gen_range(0..=road_section.direction.max_position_in_section()),
            road_section,
        }
    }

    pub fn next(&self) -> NextCarPosition {
        let next_position = self.position_in_section + 1;

        let max_position = self.road_section.direction.max_position_in_section();
        if next_position > max_position {
            // reached end of section, needs to make a decision
            let possible_decisions = self.road_section.possible_decisions();
            return NextCarPosition::MustChoose(possible_decisions);
        }

        let next = Self {
            road_section: self.road_section,
            position_in_section: next_position,
        };
        NextCarPosition::OnlyStraight(next)
    }

    pub fn take_decision(&self, decision: CarDecision) -> Self {
        let new_road_section = self.road_section.take_decision(decision).unwrap();

        Self {
            road_section: new_road_section,
            position_in_section: 0,
        }
    }
}

pub struct CarProps {
    pub agent: Box<dyn CarAgent + Send + Sync>,
    pub colour: Color,
    pub speed: usize, // ticks per movement
}

impl CarProps {
    pub fn new(agent: impl CarAgent + Send + Sync + 'static, speed: usize) -> Self {
        Self {
            agent: Box::new(agent),
            colour: Self::random_colour(),
            speed,
        }
    }

    fn random_colour() -> Color {
        let mut rng = rand::thread_rng();
        // Color {
        //     r: rng.gen(),
        //     g: rng.gen(),
        //     b: rng.gen(),
        //     a: 1.0,
        // }

        const POSSIBLE_COLOURS: &[Color] = &[BLUE, RED, ORANGE];
        *POSSIBLE_COLOURS.choose(&mut rng).unwrap()
    }
}

pub struct Car {
    pub props: CarProps,

    // variable data
    pub position: CarPosition,
    pub ticks_since_last_movement: usize,
}

impl Car {
    // speed unit: "progress" per second
    // pub const STRAIGHT_SPEED: f32 = 0.3;
    // pub const TURN_SPEED: f32 = 1.;
    // pub const SPEED: f32 = 0.3; // progress per second

    // pub fn new(position: CarPosition, agent: impl CarAgent + Send + Sync + 'static) -> Self {
    //     Self {
    //         position,
    //         agent: Box::new(agent),
    //     }
    // }

    pub fn new(props: CarProps, position: CarPosition) -> Self {
        Self {
            props,
            position,
            ticks_since_last_movement: 0,
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
    fn turn(&mut self, grid: &Grid, car: &Car) -> CarDecision;
    fn path(&self) -> Option<&Path> {
        None
    }
}

// temporary placeholder agent to put instead of the real agent
pub struct NullAgent {}

impl CarAgent for NullAgent {
    fn turn(&mut self, _grid: &Grid, _car: &Car) -> CarDecision {
        unreachable!()
    }
}

pub struct RandomTurns {}

impl CarAgent for RandomTurns {
    fn turn(&mut self, _grid: &Grid, car: &Car) -> CarDecision {
        // *options
        //     .choose(&mut rand::thread_rng())
        //     .expect("List of possible car decisions is empty")
        let options = car.position.road_section.possible_decisions();
        *options
            .choose(&mut rand::thread_rng())
            .expect("List of possible car decisions is empty")
    }
}

#[derive(Default)]
pub struct RandomDestination {
    path: Option<Path>,
}

impl CarAgent for RandomDestination {
    fn turn(&mut self, _grid: &Grid, car: &Car) -> CarDecision {
        if self.path.is_none() {
            let destination = CarPosition::random(&mut rand::thread_rng());
            let path = Path::find(car.position, destination);
            self.path = Some(path);
        }
        let path = self.path.as_mut().unwrap();
        path.pop_next_decision().unwrap_or_else(|| {
            // we already arrived, delete path and recursively call ourselves
            // to create new one
            self.path = None;
            self.turn(_grid, car)
        })
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_ref()
    }
}
