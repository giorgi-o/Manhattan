use std::collections::BinaryHeap;

use pyo3::prelude::*;

use crate::logic::{
    car::{Car, CarId, CarPassenger, CarPosition, NextCarPosition},
    ev::{ChargingStation, ChargingStationId},
    grid::Grid,
    grid_util::{GridOpts, GridStats, TickEvent},
    passenger::{Passenger, PassengerId},
    pathfinding::Path,
    util::{Direction, RoadSection},
};

use super::bridge::PyAction;

#[derive(Clone, Debug, PartialEq)]
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
    charging_stations: Vec<PyChargingStation>,

    #[pyo3(get)]
    ticks_passed: usize,
    #[pyo3(get)]
    events: PyTickEvents,
    #[pyo3(get)]
    stats: GridStats,

    car_radius: usize,
    passenger_radius: usize,
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

    fn __eq__(&self, other: &Self) -> bool {
        self == other
    }

    fn total_passenger_count(&self) -> usize {
        self.idle_passengers.len()
            + self.pov_car.as_ref().unwrap().passengers.len()
            + self
                .other_cars
                .iter()
                .map(|car| car.passengers.len())
                .sum::<usize>()
    }

    fn write_stats(&self) {
        let mut manhattan_dir = std::env::current_exe().unwrap();
        while manhattan_dir.file_name().unwrap().to_str() != Some("manhattan") {
            let has_parent = manhattan_dir.pop();
            assert!(
                has_parent,
                "Could not find manhattan/ dir! Is the .exe in there?"
            );
        }

        let stats_path = manhattan_dir.join("logs").join("stats.csv");
        if !stats_path.exists() || stats_path.metadata().unwrap().len() == 0 {
            let csv_header = self.stats.csv_header();
            std::fs::write(&stats_path, csv_header).unwrap();
        }

        let mut stats_file = std::fs::OpenOptions::new()
            .append(true)
            .open(stats_path)
            .unwrap();

        let csv_row = self.stats.csv_ify();
        std::io::Write::write_all(&mut stats_file, csv_row.as_bytes()).unwrap();
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
    #[pyo3(get)]
    in_charging_station: bool,

    // for converting to/from CarPosition
    pos_in_section: Option<usize>,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
#[pyclass]
pub enum PyCarType {
    Agent,
    Npc,
}

#[derive(Clone, Debug)]
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
    battery: f32,
    #[pyo3(get)]
    recent_actions: Vec<PyAction>,
    #[pyo3(get)]
    ticks_since_out_of_battery: usize,
    #[pyo3(get)]
    active_action: Option<PyAction>,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
#[pyclass]
pub enum PyPassengerState {
    Idle,
    Riding,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
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

#[derive(PartialEq, Eq, Clone, Default, Debug)]
#[pyclass]
pub struct PyTickEvents {
    #[pyo3(get)]
    passenger_spawned: Vec<(PyPassenger, PyCoords)>,
    #[pyo3(get)]
    car_picked_up_passenger: Vec<(PyCar, PyPassenger, PyCoords)>,
    #[pyo3(get)]
    car_dropped_off_passenger: Vec<(PyCar, PyPassenger, PyCoords)>,
    #[pyo3(get)]
    // don't store pycar for car_out_of_battery because it requires
    // building a PyCar from a CarToSpawn, which is too much effort
    // knowing that python only cares about the vec length atm.
    car_out_of_battery: Vec<(CarId, PyCoords)>,
}

#[derive(PartialEq, Clone, Debug)]
#[pyclass]
pub struct PyChargingStation {
    pub id: ChargingStationId,

    #[pyo3(get)]
    pos: PyCoords,
    #[pyo3(get)]
    capacity: usize,
    #[pyo3(get)]
    charging_speed: f32,
    #[pyo3(get)]
    cars: Vec<PyCar>,
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

        // === process charging stations ===
        let charging_stations = grid
            .charging_stations
            .values()
            .map(|station| PyChargingStation::build(station, grid))
            .collect::<Vec<_>>();

        // === process events ===
        let events = PyTickEvents::build(grid);

        // === return ===
        Self {
            opts: grid.opts.clone(),
            width: Grid::VERTICAL_ROADS,
            height: Grid::HORIZONTAL_ROADS,

            pov_car: None,
            can_turn: None,

            other_cars: cars,
            idle_passengers,
            charging_stations,

            ticks_passed,
            events,

            car_radius: grid.opts.car_radius,
            passenger_radius: grid.opts.passenger_radius,
            stats: grid.stats.clone(),
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
        let val = |passenger: &PyPassenger| pov_car.position.distance_to(passenger.pos.into());
        this.idle_passengers =
            lowest_n_sorted(this.idle_passengers.into_iter(), self.passenger_radius, val);

        // sort cars by closest to pov car
        let val = |car: &PyCar| pov_car.position.distance_to(car.pos.into());
        this.other_cars = lowest_n_sorted(this.other_cars.into_iter(), self.car_radius, val);

        // only include events by this car
        this.events
            .car_picked_up_passenger
            .retain(|(car, _, _)| car.id == pov_car.id());
        this.events
            .car_dropped_off_passenger
            .retain(|(car, _, _)| car.id == pov_car.id());
        this.events
            .car_out_of_battery
            .retain(|(car_id, _)| *car_id == pov_car.id());

        // sort charging stations by closest to pov car
        this.charging_stations
            .sort_by_cached_key(|station| pov_car.position.distance_to(station.pos.into()));

        this
    }
}

pub fn lowest_n_sorted<I, F, V>(iter: I, n: usize, mut val: F) -> Vec<I::Item>
where
    I: Iterator + std::fmt::Debug,
    I::Item: PartialEq + Eq + std::fmt::Debug,
    F: FnMut(&I::Item) -> V,
    V: Ord + PartialEq + Eq + std::fmt::Debug,
{
    // takes an iterator, get the lowest n elements and sorts them
    // at the front of the returned vector. all the other elements will be
    // there too, but beyond n elements, are not sorted.

    #[derive(PartialEq, Eq)]
    struct Item<T, V>
    where
        T: PartialEq + Eq,
        V: Ord + PartialEq + Eq,
    {
        item: T,
        value: V,
    }

    impl<T, V> PartialOrd for Item<T, V>
    where
        T: PartialEq + Eq,
        V: Ord + PartialEq + Eq,
    {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            self.value.partial_cmp(&other.value)
        }
    }

    impl<T, V> Ord for Item<T, V>
    where
        T: PartialEq + Eq,
        V: Ord + PartialEq + Eq,
    {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.value.cmp(&other.value)
        }
    }

    // tmp
    // let iter = iter.collect::<Vec<_>>();
    // // println!("Sorting lowest {n}: {iter:#?}");
    // println!(
    //     "vals: {:?}",
    //     iter.iter()
    //         .map(|i| format!("{:?}", val(i)))
    //         .collect::<Vec<_>>()
    // );
    // let iter = iter.into_iter();

    let mut heap: BinaryHeap<Item<I::Item, V>> = BinaryHeap::with_capacity(n + 1);

    let popped_elements_capacity = iter.size_hint().1.map_or(0, |c| c.saturating_sub(n));
    let mut popped_elements = Vec::with_capacity(popped_elements_capacity);

    for item in iter {
        let item = Item {
            value: val(&item),
            item,
        };
        heap.push(item);

        if heap.len() > n {
            let popped = heap.pop();
            popped_elements.push(popped.unwrap().item);
        }
    }

    let result: Vec<I::Item> = heap
        .into_sorted_vec()
        .into_iter()
        .map(|item| item.item)
        .chain(popped_elements)
        .collect();

    // tmp
    // println!("result: {result:#?}");
    // println!(
    //     "sorted vals: {:?}",
    //     result
    //         .iter()
    //         .map(|i| format!("{:?}", val(i)))
    //         .collect::<Vec<_>>()
    // );

    result
}

impl PyPassenger {
    pub fn idle(passenger: &Passenger, ticks_passed: usize) -> Self {
        let path_to_destination = Path::find(passenger.start, passenger.destination);
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
        let path_to_destination = Path::find(passenger.start, passenger.destination);
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
            in_charging_station: pos.is_at_charging_station(),

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
            in_charging_station: None,
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

        let recent_actions = car.recent_actions.iter().copied().collect();
        let ticks_since_out_of_battery = car.ticks_since_out_of_battery;

        Self {
            id: car.id(),
            ty,
            pos: car.position.into(),
            passengers,
            battery: car.battery.get(),
            recent_actions,
            ticks_since_out_of_battery,
            active_action: car.active_action,
        }
    }
}

impl PyChargingStation {
    pub fn build(station: &ChargingStation, grid: &Grid) -> Self {
        let cars = station
            .cars
            .iter()
            .map(|car_id| grid.car(*car_id))
            .map(|car| PyCar::build(car, grid.ticks_passed))
            .collect::<Vec<_>>();

        Self {
            id: station.id,
            pos: station.entrance.into(),
            capacity: station.capacity,
            charging_speed: station.charging_speed.get(),
            cars,
        }
    }
}

impl PyTickEvents {
    pub fn build(grid: &Grid) -> Self {
        let mut this = Self {
            passenger_spawned: vec![],
            car_picked_up_passenger: vec![],
            car_dropped_off_passenger: vec![],
            car_out_of_battery: vec![],
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

                TickEvent::CarOutOfBattery(car_id, out_of_battery_pos) => {
                    let py_pos = (*out_of_battery_pos).into();
                    this.car_out_of_battery.push((*car_id, py_pos));
                }
            }
        }

        this
    }
}

impl PartialEq for PyCar {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for PyCar {}

#[pymethods]
impl PyCar {
    fn __eq__(&self, other: &PyCar) -> bool {
        self == other
    }
}

#[pymethods]
impl PyChargingStation {
    fn is_full(&self) -> bool {
        self.cars.len() == self.capacity
    }
}
