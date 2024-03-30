use pyo3::prelude::*;

use crate::logic::{
    car::{Car, CarPassenger, CarPosition},
    grid::{Direction, Grid, GridOpts, RoadSection},
    passenger::{Passenger, PassengerId},
    pathfinding::Path,
};

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
    cars: Vec<PyCar>,
    #[pyo3(get)]
    idle_passengers: Vec<PyPassenger>,

    ticks_passed: usize,
}

#[pymethods]
impl PyGridState {
    fn __repr__(&self) -> String {
        format!(
            "<PyGridState cars={} passengers={} ticks_passed={}>",
            self.cars.len(),
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
    #[pyo3(get)]
    ty: PyCarType,
    #[pyo3(get)]
    pos: PyCoords,
    #[pyo3(get)]
    passengers: Vec<PyPassenger>,
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
            .cars
            .iter()
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
            cars,
            idle_passengers,

            ticks_passed,
        }
    }

    pub fn with_pov(&self, pov_car: &Car) -> Self {
        let mut this = self.clone();
        this.pov_car = Some(PyCar::build(pov_car, self.ticks_passed));

        // sort passengers by closest to car
        this.idle_passengers.sort_by_cached_key(|passenger| {
            let path = Path::find(pov_car.position, passenger.pos.into(), Grid::CAR_SPEED);
            path.cost
        });

        // do not include the pov car in cars list
        this.cars.retain(|car| pov_car.position != car.pos.into());

        // sort cars by closest to pov car
        this.cars.sort_by_cached_key(|car| {
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

// impl From<RoadSection> for PyCoords {
//     fn from(section: RoadSection) -> Self {
//         Self {
//             direction: section.direction,
//             road: section.road(),
//             section: section.section(),

//             pos_in_section: None,
//         }
//     }
// }

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

        Self {
            ty,
            pos: car.position.into(),
            passengers,
        }
    }
}
