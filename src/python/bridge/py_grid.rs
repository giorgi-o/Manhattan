use pyo3::prelude::*;

use crate::logic::{
    car::{Car, CarId, CarPassenger, CarPosition, NextCarPosition},
    grid::{Grid, GridOpts, TickEvent},
    passenger::{Passenger, PassengerId},
    pathfinding::Path,
    util::{Direction, RoadSection},
};

use super::bridge::PyAction;

#[derive(Clone, Debug)]
#[pyclass]
pub struct PyGridState {
    #[pyo3(get)]
    opts: GridOpts,

    #[pyo3(get)]
    width: usize,
    #[pyo3(get)]
    height: usize,

    #[pyo3(get)]
    pov_car: Option<PyCar>,
    #[pyo3(get)]
    can_turn: Option<bool>, // whether the car can choose this tick

    #[pyo3(get)]
    other_cars: Vec<PyCar>,
    #[pyo3(get)]
    idle_passengers: Vec<PyPassenger>,

    #[pyo3(get)]
    ticks_passed: usize,
    #[pyo3(get)]
    events: PyTickEvents,
}

#[pymethods]
impl PyGridState {
    fn __repr__(&self) -> String {
        format!(
            "<PyGridState cars={} passengers={} ticks_passed={}>",
            self.other_cars.len(),
            self.idle_passengers.len(),
            self.ticks_passed
        )
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
#[pyclass]
pub struct PyCoords {
    #[pyo3(get)]
    direction: Direction,
    #[pyo3(get)]
    road: usize,
    #[pyo3(get)]
    section: usize,

    // for converting to/from CarPosition
    pos_in_section: Option<usize>,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
#[pyclass]
pub enum PyCarType {
    Agent,
    Npc,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
#[pyclass]
pub struct PyCar {
    pub id: CarId,

    #[pyo3(get)]
    ty: PyCarType,
    #[pyo3(get)]
    pos: PyCoords,
    #[pyo3(get)]
    passengers: Vec<PyPassenger>,
    #[pyo3(get)]
    recent_actions: Vec<PyAction>,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
#[pyclass]
pub enum PyPassengerState {
    Idle,
    Riding,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
#[pyclass]
pub struct PyPassenger {
    pub id: PassengerId,
    #[pyo3(get)]
    pos: PyCoords,
    #[pyo3(get)]
    destination: PyCoords,
    #[pyo3(get)]
    state: PyPassengerState,
    #[pyo3(get)]
    ticks_since_request: usize,
    #[pyo3(get)]
    distance_to_destination: usize,
}

#[derive(PartialEq, Eq, Hash, Clone, Default, Debug)]
#[pyclass]
pub struct PyTickEvents {
    #[pyo3(get)]
    passenger_spawned: Vec<(PyPassenger, PyCoords)>,
    #[pyo3(get)]
    car_picked_up_passenger: Vec<(PyCar, PyPassenger, PyCoords)>,
    #[pyo3(get)]
    car_dropped_off_passenger: Vec<(PyCar, PyPassenger, PyCoords)>,
}

impl PyGridState {
    pub fn has_pov(&self) -> bool {
        self.pov_car.is_some()
    }

    pub fn build(grid: &Grid) -> Self {
        let ticks_passed = grid.ticks_passed;

        // === process idle passengers ===
        let idle_passengers = grid
            .waiting_passengers()
            .map(|passenger| PyPassenger::idle(passenger, ticks_passed))
            .collect::<Vec<_>>();

        //  === process cars ===
        let cars = grid
            .cars()
            .map(|car| PyCar::build(car, ticks_passed))
            .collect::<Vec<_>>();

        // tmp dbg
        // println!("idle passengers: {:?}", idle_passengers);
        // println!("cars: {:?}", cars);

        // === return ===
        Self {
            opts: grid.opts,
            width: Grid::VERTICAL_ROADS,
            height: Grid::HORIZONTAL_ROADS,

            pov_car: None,
            can_turn: None,

            other_cars: cars,
            idle_passengers,

            ticks_passed,
            events: PyTickEvents::default(),
        }
    }

    pub fn with_pov(&self, pov_car: &Car) -> Self {
        let mut this = self.clone();

        // take the pov car out of the other cars vec
        let pov_car_index = this
            .other_cars
            .iter()
            .position(|car| car.id == pov_car.id())
            .expect("pov car not in self.other_cars");
        let py_pov_car = this.other_cars.swap_remove(pov_car_index);
        this.pov_car = Some(py_pov_car);

        // calculate whether the car's action this tick has an effect
        let next_position = pov_car.position.next();
        let can_turn = matches!(next_position, NextCarPosition::MustChoose);
        this.can_turn = Some(can_turn);

        // sort passengers by closest to car
        this.idle_passengers.sort_by_cached_key(|passenger| {
            let path = pov_car.find_path(passenger.pos.into());
            path.cost
        });

        // sort cars by closest to pov car
        this.other_cars.sort_by_cached_key(|car| {
            Path::distance(pov_car.position, car.pos.into(), Grid::CAR_SPEED);
        });

        this
    }
}

impl PyPassenger {
    pub fn idle(passenger: &Passenger, ticks_passed: usize) -> Self {
        let path_to_destination =
            Path::find(passenger.start, passenger.destination, Grid::CAR_SPEED);
        let distance_to_destination = path_to_destination.cost;

        Self {
            id: passenger.id,
            pos: passenger.start.into(),
            destination: passenger.destination.into(),
            state: PyPassengerState::Idle,
            ticks_since_request: ticks_passed - passenger.start_tick,
            distance_to_destination,
        }
    }

    pub fn riding(passenger: &Passenger, car: &Car, ticks_passed: usize) -> Self {
        let path_to_destination =
            Path::find(passenger.start, passenger.destination, Grid::CAR_SPEED);
        let distance_to_destination = path_to_destination.cost;

        Self {
            id: passenger.id,
            pos: car.position.into(),
            destination: passenger.destination.into(),
            state: PyPassengerState::Riding,
            ticks_since_request: ticks_passed - passenger.start_tick,
            distance_to_destination,
        }
    }
}

impl From<CarPosition> for PyCoords {
    fn from(pos: CarPosition) -> Self {
        Self {
            direction: pos.road_section.direction,
            road: pos.road_section.road(),
            section: pos.road_section.section(),

            pos_in_section: Some(pos.position_in_section),
        }
    }
}

impl From<PyCoords> for RoadSection {
    fn from(pos: PyCoords) -> Self {
        RoadSection {
            direction: pos.direction,
            road_index: pos.road as isize,
            section_index: pos.section as isize,
        }
    }
}

impl From<PyCoords> for CarPosition {
    fn from(pos: PyCoords) -> Self {
        CarPosition {
            road_section: pos.into(),
            position_in_section: pos.pos_in_section.expect("No car pos in section info"),
        }
    }
}

impl PyCar {
    pub fn build(car: &Car, ticks_passed: usize) -> Self {
        let ty = match car.props.agent.is_npc() {
            true => PyCarType::Npc,
            false => PyCarType::Agent,
        };

        // process passengers in car
        let mut passengers = Vec::with_capacity(car.passengers.len());
        for passenger in &car.passengers {
            let CarPassenger::DroppingOff(passenger) = passenger else {
                continue; // only process passengers currently in the car
            };

            let py_passenger = PyPassenger::riding(passenger, car, ticks_passed);
            passengers.push(py_passenger);
        }

        let recent_actions = car.recent_actions.iter().cloned().collect();

        Self {
            id: car.id(),
            ty,
            pos: car.position.into(),
            passengers,
            recent_actions,
        }
    }
}

impl PyTickEvents {
    pub fn build(grid: &Grid) -> Self {
        let mut this = Self {
            passenger_spawned: vec![],
            car_picked_up_passenger: vec![],
            car_dropped_off_passenger: vec![],
        };

        for event in &grid.tick_events {
            match event {
                TickEvent::PassengerSpawned(passenger_id) => {
                    let passenger = grid
                        .get_idle_passenger(*passenger_id)
                        .expect("Passenger spawned but not found");

                    let py_passenger = PyPassenger::idle(passenger, grid.ticks_passed);
                    let py_pos = py_passenger.pos;
                    this.passenger_spawned.push((py_passenger, py_pos));
                }

                TickEvent::PassengerPickedUp(car_id, passenger_id) => {
                    let car = grid.car(*car_id);
                    let passenger = car
                        .passengers
                        .iter()
                        .find_map(|p| {
                            if let CarPassenger::DroppingOff(p) = p {
                                return (p.id == *passenger_id).then_some(p);
                            };
                            None
                        })
                        .expect("Passenger picked up but not found in car");

                    let py_car = PyCar::build(car, grid.ticks_passed);
                    let py_passenger = PyPassenger::riding(passenger, car, grid.ticks_passed);
                    let py_pos = py_passenger.pos;
                    this.car_picked_up_passenger
                        .push((py_car, py_passenger, py_pos));
                }

                TickEvent::PassengerDroppedOff(car_id, passenger) => {
                    let car = grid.car(*car_id);

                    let py_passenger = PyPassenger::riding(passenger, car, grid.ticks_passed);
                    let py_car = PyCar::build(car, grid.ticks_passed);
                    let py_pos = py_passenger.pos;

                    this.car_dropped_off_passenger
                        .push((py_car, py_passenger, py_pos));
                }
            }
        }

        this
    }
}
