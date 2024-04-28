use core::panic;
use std::{io::Write, mem};

use macroquad::color::*;
use pyo3::prelude::*;
use rand::Rng;

use crate::{
    logic::car::NextCarPosition,
    python::bridge::{bridge::PythonAgentWrapper, py_grid::PyGridState},
};

use super::{
    car::{Car, CarDecision, CarId, CarPassenger, CarPosition, CarProps, CarToSpawn},
    car_agent::{NullAgent, PythonAgent, RandomTurns},
    ev::{ChargingStation, ChargingStationId},
    grid::Grid,
    passenger::{Passenger, PassengerId},
    util::{hashmap_with_capacity, Direction, HashMap, HashSet, Orientation, RoadSection},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[pyclass]
pub enum LightState {
    Red,
    Green,
}

impl LightState {
    pub fn toggle(&mut self) {
        *self = match self {
            LightState::Red => LightState::Green,
            LightState::Green => LightState::Red,
        }
    }

    pub fn random(mut rng: impl Rng) -> Self {
        match rng.gen() {
            true => LightState::Green,
            false => LightState::Red,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[pyclass]
pub struct TrafficLight {
    #[pyo3(get)]
    pub toggle_every_ticks: usize,
    #[pyo3(get)]
    pub state: LightState,
    #[pyo3(get)]
    pub ticks_left: usize,
}

impl TrafficLight {
    pub fn tick(&mut self) {
        if self.ticks_left > 0 {
            self.ticks_left -= 1;
        } else {
            self.state.toggle();
            self.ticks_left = self.toggle_every_ticks;
        }
    }

    // see what the light will be like in X ticks
    pub fn time_travel(&self, ticks: usize) -> Self {
        let state_changes = ticks / self.toggle_every_ticks;
        let remainder = ticks % self.toggle_every_ticks;

        let mut new_state = self.state;
        if state_changes % 2 == 1 {
            new_state.toggle();
        }

        let mut ticks_left = self.ticks_left;
        if remainder <= ticks_left {
            ticks_left -= remainder;
        } else {
            new_state.toggle();
            ticks_left = self.toggle_every_ticks - remainder;
        }

        Self {
            toggle_every_ticks: self.toggle_every_ticks,
            state: new_state,
            ticks_left,
        }
    }
}

pub enum TickEvent {
    PassengerSpawned(PassengerId),
    PassengerPickedUp(CarId, PassengerId),
    PassengerDroppedOff(CarId, Passenger),
    CarOutOfBattery(CarId, CarPosition),
}

#[pyclass]
#[derive(Default, Debug, Clone, PartialEq)]
pub struct GridStats {
    pub ticks: usize,

    pub passenger_spawns: usize,
    pub passenger_pickups: usize,
    pub passenger_dropoffs: usize,

    pub pick_up_requests: usize,
    pub drop_off_requests: usize,
    pub charge_requests: usize,
    pub head_towards_requests: usize,

    pub enter_charging_stations: usize,
    pub out_of_battery: usize,

    pub ticks_with_n_passengers: Vec<usize>,
    pub ticks_picking_up_n_closest_passenger: Vec<usize>,
    pub ticks_dropping_off_n_closest_passenger: Vec<usize>,
}

#[pymethods]
impl GridStats {
    const MAX_PASSENGERS_PER_CAR: usize = 4;
    const MAX_PASSENGER_RADIUS: usize = 5;

    pub fn csv_header(&self) -> String {
        let headers = vec![
            "ticks",
            "passenger_spawns",
            "passenger_pickups",
            "passenger_dropoffs",
            "pick_up_requests",
            "drop_off_requests",
            "charge_requests",
            "head_towards_requests",
            "enter_charging_stations",
            "out_of_battery",
        ];
        let mut headers = headers.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        for n in 0..=Self::MAX_PASSENGERS_PER_CAR {
            headers.push(format!("ticks_with_{}_passengers", n));
        }
        for n in 0..=Self::MAX_PASSENGER_RADIUS {
            headers.push(format!("ticks_picking_up_{}_closest_passenger", n));
        }
        for n in 0..=Self::MAX_PASSENGER_RADIUS {
            headers.push(format!("ticks_dropping_off_{}_closest_passenger", n));
        }

        headers.join(",") + "\n"
    }

    pub fn csv_ify(&self) -> String {
        let mut values = vec![
            self.ticks.to_string(),
            self.passenger_spawns.to_string(),
            self.passenger_pickups.to_string(),
            self.passenger_dropoffs.to_string(),
            self.pick_up_requests.to_string(),
            self.drop_off_requests.to_string(),
            self.charge_requests.to_string(),
            self.head_towards_requests.to_string(),
            self.enter_charging_stations.to_string(),
            self.out_of_battery.to_string(),
        ];

        for n in 0..=Self::MAX_PASSENGERS_PER_CAR {
            let value = self.ticks_with_n_passengers.get(n).unwrap_or(&0);
            values.push(value.to_string());
        }
        for n in 0..=Self::MAX_PASSENGER_RADIUS {
            let value = self
                .ticks_picking_up_n_closest_passenger
                .get(n)
                .unwrap_or(&0);
            values.push(value.to_string());
        }
        for n in 0..=Self::MAX_PASSENGER_RADIUS {
            let value = self
                .ticks_dropping_off_n_closest_passenger
                .get(n)
                .unwrap_or(&0);
            values.push(value.to_string());
        }

        values.join(",") + "\n"
    }
}

#[derive(Debug, Clone, PartialEq)]
#[pyclass]
// Not like a grid event, which is a record of something that
// happened during the episode.
// this is dictated by python, and tells us if/when to cause a
// surge of passengers to spawn in a particular area, or all
// wanting to go to a particular area.
pub struct PassengerEvent {
    // coords are f32 instead of usize for 2 reasons:
    // 1. they will be compared to the output of section.cartesian_coords()
    //    which returns f32s
    // 2. python doesn't care anyway
    pub start_area: (f32, f32, f32, f32), // (x1, y1, x2, y2)
    pub destination_area: (f32, f32, f32, f32), // (x1, y1, x2, y2)
    // note: start/end are both inclusive btw,and if start tick is 0,
    // this includes the passengers spawned at the start (tick -1)
    pub between_ticks: (Option<usize>, Option<usize>), // (start tick, end tick)
    pub spawn_rate: Option<f32>,
}

#[pymethods]
impl PassengerEvent {
    #[new]
    fn new(
        start_area: (f32, f32, f32, f32),
        destination_area: (f32, f32, f32, f32),
        between_ticks: (Option<usize>, Option<usize>),
        spawn_rate: Option<f32>,
    ) -> Self {
        let start_area = wrap_negative_coords(start_area);
        let destination_area = wrap_negative_coords(destination_area);

        let (sx1, sy1, sx2, sy2) = start_area;
        let (dx1, dy1, dx2, dy2) = destination_area;
        assert!(sx1 <= sx2 && sy1 <= sy2 && dx1 <= dx2 && dy1 <= dy2);

        Self {
            start_area,
            destination_area,
            between_ticks,
            spawn_rate,
        }
    }
}

fn wrap_negative_coords(coords: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    let (x1, y1, x2, y2) = coords;
    let wrap = |coord: f32, max| match coord.is_sign_negative() {
        true => max + coord,
        false => coord,
    };

    let max_x = Grid::VERTICAL_ROADS as f32 - 1.0;
    let max_y = Grid::HORIZONTAL_ROADS as f32 - 1.0;
    (
        wrap(x1, max_x),
        wrap(y1, max_y),
        wrap(x2, max_x),
        wrap(y2, max_y),
    )
}

#[test]
fn test_wrap_negative_coords() {
    for direction in [Direction::Down, Direction::Right] {
        let max_road_section = RoadSection::get(
            direction,
            direction.max_road_index(),
            direction.max_section_index(),
        );
        let max_checkerboard_coords = max_road_section.checkerboard_coords();

        let coords = (0.0, 0.0, -0.0, -0.0);
        let wrapped = wrap_negative_coords(coords);

        assert!(
            max_checkerboard_coords.0 == wrapped.2 || wrapped.2 - 0.5 == max_checkerboard_coords.0
        );
        assert!(
            max_checkerboard_coords.1 == wrapped.3 || wrapped.3 - 0.5 == max_checkerboard_coords.1
        );
    }
}

#[derive(Debug, Clone, PartialEq)]
#[pyclass]
pub struct GridOpts {
    #[pyo3(get)]
    pub initial_passenger_count: u32, // number of passengers on the grid at the start
    #[pyo3(get)]
    pub passenger_spawn_rate: f32, // chance of spawning a new passenger per tick
    #[pyo3(get)]
    pub max_passengers: usize,
    #[pyo3(get)]
    pub agent_car_count: u32,
    #[pyo3(get)]
    pub npc_car_count: u32,
    #[pyo3(get)]
    pub passengers_per_car: usize,
    #[pyo3(get)]
    pub charging_stations: Vec<CarPosition>,
    #[pyo3(get)]
    pub charging_station_capacity: usize,
    #[pyo3(get)]
    pub discharge_rate: f32,
    #[pyo3(get)]
    pub car_radius: usize,
    #[pyo3(get)]
    pub passenger_radius: usize,
    #[pyo3(get)]
    pub passenger_events: Vec<PassengerEvent>,
    #[pyo3(get)]
    pub deterministic_mode: bool,
    #[pyo3(get)]
    pub verbose: bool,
}

#[pymethods]
impl GridOpts {
    #[new]
    fn new(
        initial_passenger_count: u32,
        passenger_spawn_rate: f32,
        max_passengers: usize,
        agent_car_count: u32,
        npc_car_count: u32,
        passengers_per_car: usize,
        charging_stations: Vec<CarPosition>,
        charging_station_capacity: usize,
        discharge_rate: f32,
        car_radius: usize,
        passenger_radius: usize,
        passenger_events: Vec<PassengerEvent>,
        deterministic_mode: bool,
        verbose: bool,
    ) -> Self {
        Self {
            initial_passenger_count,
            passenger_spawn_rate,
            max_passengers,
            agent_car_count,
            npc_car_count,
            passengers_per_car,
            charging_stations,
            charging_station_capacity,
            discharge_rate,
            car_radius,
            passenger_radius,
            passenger_events,
            deterministic_mode,
            verbose,
        }
    }
}
