use std::{
    collections::VecDeque,
    hash::Hash,
    sync::atomic::{AtomicUsize, Ordering},
};

use macroquad::color::Color;
use pyo3::prelude::*;
use rand::Rng;

use crate::{python::bridge::bridge::PyAction, render::car::CarRenderer};

use super::{
    car_agent::CarAgent,
    ev::{BatteryPercent, ChargingStation, ChargingStationId},
    passenger::{Passenger, PassengerId},
    pathfinding::Path,
    util::{Direction, RoadSection},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[pyclass]
pub struct CarId(usize);

impl CarId {
    pub fn next() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        Self(NEXT_ID.fetch_add(1, Ordering::SeqCst))
    }
}

#[derive(Clone, Copy, Debug)]
#[pyclass]
pub struct CarPosition {
    #[pyo3(get)]
    pub road_section: RoadSection,
    #[pyo3(get)]
    pub position_in_section: usize, // higher = further along

    #[pyo3(get)]
    pub in_charging_station: Option<ChargingStationId>, // if some, overrides the two above
}

// manually implement hash + partialeq, because if two cars are in the same
// charging station, they should be equal no matter what the other fields say

impl PartialEq for CarPosition {
    fn eq(&self, other: &Self) -> bool {
        match (self.in_charging_station, other.in_charging_station) {
            (Some(self_id), Some(other_id)) => self_id == other_id,
            (Some(_), None) | (None, Some(_)) => false,
            (None, None) => {
                self.road_section == other.road_section
                    && self.position_in_section == other.position_in_section
            }
        }
    }
}

impl Eq for CarPosition {}

impl Hash for CarPosition {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if let Some(in_charging_station) = self.in_charging_station {
            in_charging_station.hash(state);
        } else {
            self.in_charging_station.hash(state);
            self.road_section.hash(state);
            self.position_in_section.hash(state);
        }
    }
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
            in_charging_station: None,
        }
    }

    pub fn at_charging_station(charging_station: &ChargingStation) -> Self {
        let entrance = charging_station.entrance;
        Self {
            in_charging_station: Some(charging_station.id),

            road_section: entrance.road_section,
            position_in_section: entrance.position_in_section,
        }
    }

    pub fn is_at_charging_station(&self) -> bool {
        self.in_charging_station.is_some()
    }

    pub fn next(&self) -> NextCarPosition {
        if self.in_charging_station.is_some() {
            // needs to choose between leaving or keep charging
            return NextCarPosition::MustChoose;
        }

        let next_position = self.position_in_section + 1;

        let max_position = self.road_section.direction.max_position_in_section();
        if next_position > max_position {
            // reached end of section, needs to make a decision
            return NextCarPosition::MustChoose;
        }

        let next = Self {
            road_section: self.road_section,
            position_in_section: next_position,
            in_charging_station: self.in_charging_station,
        };
        NextCarPosition::OnlyStraight(next)
    }

    pub fn other_side_of_road(&self) -> Self {
        assert!(!self.is_at_charging_station());

        let old_road_section = self.road_section;
        let road_section = RoadSection::get_raw(
            old_road_section.direction.inverted(),
            old_road_section.road_index,
            old_road_section.section_index,
        );

        let mirrored_section_pos = (0..old_road_section.direction.max_position_in_section())
            .rev()
            .nth(self.position_in_section)
            .unwrap();

        Self {
            road_section,
            position_in_section: mirrored_section_pos,
            in_charging_station: None,
        }
    }

    pub fn possible_decisions(&self) -> Vec<CarDecision> {
        match self.in_charging_station {
            Some(_) => vec![
                CarDecision::ChargeBattery,
                CarDecision::TurnLeft,
                CarDecision::TurnRight,
            ],
            None => self.road_section.possible_decisions(),
        }
    }

    pub fn take_decision(&self, decision: CarDecision) -> Self {
        if self.is_at_charging_station() {
            return match decision {
                CarDecision::ChargeBattery => self.clone(), // stay still
                CarDecision::TurnLeft | CarDecision::TurnRight => {
                    self.leave_charging_station(decision)
                }

                // if the car is leaving the station, it must choose whether to go left or right
                CarDecision::GoStraight => panic!("car at charging station cannot GoStraight"),
            };
        } else if decision == CarDecision::ChargeBattery {
            panic!("car cannot ChargeBattery if not at a charging station");
        }

        let new_road_section = self.road_section.take_decision(decision).unwrap();
        Self {
            road_section: new_road_section,
            position_in_section: 0,
            in_charging_station: self.in_charging_station,
        }
    }

    pub fn is_at_intersection(&self) -> bool {
        self.position_in_section == self.road_section.direction.max_position_in_section()
    }

    pub fn path_to(self, other: CarPosition) -> Path {
        Path::find(self, other, CarProps::SPEED)
    }

    pub fn distance_to(self, other: CarPosition) -> usize {
        if self == other {
            return 0;
        }

        if self.road_section == other.road_section
            && self.position_in_section <= other.position_in_section
        {
            // not gonna bother adding 1 cost for in/out charging station
            return other.position_in_section - self.position_in_section;
        }

        self.path_to(other).cost
    }

    pub fn leave_charging_station(&self, decision: CarDecision) -> Self {
        assert!(self.is_at_charging_station());
        assert!(decision == CarDecision::TurnLeft || decision == CarDecision::TurnRight);

        let charging_station = self.in_charging_station.unwrap();
        let charging_station_entrance = charging_station.entrance();

        let is_turning_left = decision == CarDecision::TurnLeft;
        let drive_on_left_side = CarRenderer::ENGLAND_MODE;

        let new_position = match is_turning_left == drive_on_left_side {
            true => charging_station_entrance,
            false => charging_station_entrance.other_side_of_road(),
        };

        new_position
    }
}

#[pymethods]
impl CarPosition {
    #[new]
    pub fn new(
        direction: Direction,
        road_index: usize,
        section_index: usize,
        position_in_section: usize,
    ) -> Self {
        Self {
            road_section: RoadSection::get(direction, road_index, section_index),
            position_in_section,
            in_charging_station: None,
        }
    }
}

pub struct CarProps {
    pub id: CarId,
    pub agent: Box<dyn CarAgent>,
    pub colour: Color,
    pub speed: usize,        // ticks per movement
    pub discharge_rate: f32, // percent per tick
}

impl CarProps {
    pub const SPEED: usize = 3;

    pub fn new(
        agent: impl CarAgent + 'static,
        speed: usize,
        discharge_rate: f32,
        colour: Color,
    ) -> Self {
        Self {
            id: CarId::next(),
            agent: Box::new(agent),
            colour,
            speed,
            discharge_rate,
        }
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
    pub ticks_until_next_movement: usize,
    pub passengers: Vec<CarPassenger>,
    pub battery: BatteryPercent,
    pub recent_actions: VecDeque<PyAction>,
}

impl Car {
    const RECENT_ACTIONS_LEN: usize = 5;

    pub fn new(props: CarProps, position: CarPosition, battery: f32) -> Self {
        Self {
            ticks_until_next_movement: props.speed,
            props,
            position,
            passengers: vec![],
            battery: BatteryPercent::new(battery),
            recent_actions: VecDeque::with_capacity(Self::RECENT_ACTIONS_LEN),
        }
    }

    pub fn id(&self) -> CarId {
        self.props.id
    }

    pub fn find_path(&self, destination: CarPosition) -> Path {
        Path::find(self.position, destination, self.props.speed)
    }

    pub fn next_position(
        &self,
        decision: CarDecision,
        next_to_cs: Option<&ChargingStation>,
    ) -> CarPosition {
        // note: does NOT update position! only calculates the next position

        // cars can only move every "speed" ticks
        if self.ticks_until_next_movement > 0 {
            return self.position;
        }

        // if we are next to the charging station and we want
        // to go there
        if decision == CarDecision::ChargeBattery {
            let charging_station =
                next_to_cs.expect("Tried to ChargeBattery when not next to a charging station");

            return CarPosition::at_charging_station(charging_station);
        }

        // calculate next position, using decision if needed
        let old_position = self.position;
        let next_position = self.position.next();
        let next_position = match next_position {
            NextCarPosition::OnlyStraight(next) => next,
            NextCarPosition::MustChoose => old_position.take_decision(decision),
        };

        assert_ne!(old_position, next_position, "car turned but stayed still");
        next_position
    }

    pub fn took_action(&mut self, action: PyAction) {
        if self.recent_actions.len() >= Self::RECENT_ACTIONS_LEN {
            self.recent_actions.pop_back();
        }

        self.recent_actions.push_front(action);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CarDecision {
    TurnLeft,
    GoStraight,
    TurnRight,
    ChargeBattery,
    // if not in charging station, ChargeBattery is invalid
    // if in charging station, GoStraight is invalid
}

pub struct CarToSpawn {
    pub props: CarProps,

    // None = pick a random position (or out_of_battery)
    pub position: Option<CarPosition>,

    // if the car ran out of battery, it should spawn at the nearest
    // charging station that has space.
    // if this is some, try to spawn it at the closest charging station
    // and keep the passengers (lol)
    pub out_of_battery: Option<(CarPosition, Vec<CarPassenger>)>,
}

impl CarToSpawn {
    pub fn position<F>(&self, mut rng: impl Rng, pos_taken: F) -> CarPosition
    where
        F: Fn(&CarPosition) -> bool,
    {
        assert!(
            self.out_of_battery.is_none(),
            "Tried to get CarToSpawn.position() when it ran out of battery"
        );

        // self.position.unwrap_or_else(|| CarPosition::random(rng))
        if let Some(position) = &self.position {
            if !pos_taken(position) {
                return *position;
            }
        }

        // position is None or is already occupied.
        // generate a new one
        for _ in 0..1000 {
            let position = CarPosition::random(&mut rng);
            if !pos_taken(&position) {
                return position;
            }
        }

        panic!("Could not find a random position to spawn car at");
    }
}
