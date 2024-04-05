use std::{collections::VecDeque, sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
}};

use macroquad::color::Color;
use pyo3::prelude::*;
use rand::{seq::SliceRandom, Rng};

use crate::python::bridge::{
    bridge::{PyAction, PythonAgentWrapper},
    py_grid::{PyCar, PyGridState},
};

use super::{
    car_agent::CarAgent,
    grid::Grid,
    passenger::{Passenger, PassengerId},
    pathfinding::Path,
    util::RoadSection,
};

use macroquad::color::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CarId(usize);

impl CarId {
    pub fn next() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        Self(NEXT_ID.fetch_add(1, Ordering::SeqCst))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[pyclass]
pub struct CarPosition {
    #[pyo3(get)]
    pub road_section: RoadSection,
    #[pyo3(get)]
    pub position_in_section: usize, // higher = further along
}

pub enum NextCarPosition {
    OnlyStraight(CarPosition),
    MustChoose,
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
            return NextCarPosition::MustChoose;
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
    pub id: CarId,
    pub agent: Box<dyn CarAgent>,
    pub colour: Color,
    pub speed: usize, // ticks per movement
}

impl CarProps {
    pub const SPEED: usize = 3;

    pub fn new(agent: impl CarAgent + 'static, speed: usize, colour: Color) -> Self {
        Self {
            id: CarId::next(),
            agent: Box::new(agent),
            colour,
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

        const POSSIBLE_COLOURS: &[Color] = &[BLUE, RED, PURPLE];
        *POSSIBLE_COLOURS.choose(&mut rng).unwrap()
    }
}

#[derive(Debug)]
pub enum CarPassenger {
    PickingUp(PassengerId),
    DroppingOff(Passenger),
}

impl CarPassenger {
    pub fn is_dropping_off(&self) -> bool {
        matches!(self, Self::DroppingOff(_))
    }

    pub fn is_id(&self, id: PassengerId) -> bool {
        match self {
            Self::PickingUp(passenger_id) => *passenger_id == id,
            Self::DroppingOff(passenger) => passenger.id == id,
        }
    }
}

pub struct Car {
    pub props: CarProps,

    // variable data
    pub position: CarPosition,
    pub ticks_since_last_movement: usize,
    pub passengers: Vec<CarPassenger>,
    pub recent_actions: VecDeque<PyAction>,
}

impl Car {
    const RECENT_ACTIONS_LEN: usize = 5;

    pub fn new(props: CarProps, position: CarPosition) -> Self {
        Self {
            props,
            position,
            ticks_since_last_movement: 0,
            passengers: vec![],
            recent_actions: VecDeque::with_capacity(Self::RECENT_ACTIONS_LEN),
        }
    }

    pub fn id(&self) -> CarId {
        self.props.id
    }

    pub fn find_path(&self, destination: CarPosition) -> Path {
        Path::find(self.position, destination, self.props.speed)
    }

    pub fn took_action(&mut self, action: PyAction) {
        if self.recent_actions.len() >= Self::RECENT_ACTIONS_LEN {
            self.recent_actions.pop_front();
        }

        self.recent_actions.push_back(action);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CarDecision {
    TurnLeft,
    GoStraight,
    TurnRight,
}
