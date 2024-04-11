use std::mem;

use macroquad::color::*;
use pyo3::prelude::*;
use rand::Rng;

use crate::{
    logic::car::NextCarPosition,
    python::bridge::{bridge::PythonAgentWrapper, py_grid::PyGridState},
};

use super::{
    car::{Car, CarDecision, CarId, CarPassenger, CarPosition, CarProps},
    car_agent::{NullAgent, PythonAgent, RandomTurns},
    passenger::{Passenger, PassengerId},
    util::{hashmap_with_capacity, HashMap, HashSet, Orientation, RoadSection},
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
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[pyclass]
pub struct GridOpts {
    #[pyo3(get)]
    pub initial_passenger_count: u32, // number of passengers on the grid at the start
    #[pyo3(get)]
    pub passenger_spawn_rate: f32, // chance of spawning a new passenger per tick
    #[pyo3(get)]
    pub agent_car_count: u32,
    #[pyo3(get)]
    pub npc_car_count: u32,
    #[pyo3(get)]
    pub passengers_per_car: usize,
    #[pyo3(get)]
    pub verbose: bool,
}

#[pymethods]
impl GridOpts {
    #[new]
    fn new(
        initial_passenger_count: u32,
        passenger_spawn_rate: f32,
        agent_car_count: u32,
        npc_car_count: u32,
        passengers_per_car: usize,
        verbose: bool,
    ) -> Self {
        Self {
            initial_passenger_count,
            passenger_spawn_rate,
            agent_car_count,
            npc_car_count,
            passengers_per_car,
            verbose,
        }
    }
}

pub struct Grid {
    pub opts: GridOpts,

    pub cars: HashMap<CarId, Car>,
    car_positions: HashMap<CarPosition, CarId>,

    pub waiting_passengers: HashMap<PassengerId, Passenger>,
    waiting_passenger_positions: HashMap<CarPosition, PassengerId>,

    // None position = random spawn point
    pub cars_to_spawn: Vec<(CarProps, Option<CarPosition>)>,

    pub traffic_lights: HashMap<RoadSection, TrafficLight>,

    pub ticks_passed: usize,

    pub tick_state: Option<PyGridState>,
    pub tick_events: Vec<TickEvent>,
}

impl Grid {
    pub const HORIZONTAL_ROADS: usize = 5;
    pub const VERTICAL_ROADS: usize = 7;
    pub const HORIZONTAL_SECTION_SLOTS: usize = 5;
    pub const VERTICAL_SECTION_SLOTS: usize = 5;

    pub const TRAFFIC_LIGHT_TOGGLE_TICKS: usize = 60; // 3s at 20TPS

    pub const CAR_SPEED: usize = 3;

    // pub const MAX_TOTAL_PASSENGERS: usize = Self::HORIZONTAL_ROADS * Self::VERTICAL_ROADS;
    // pub const MAX_WAITING_PASSENGERS: usize = Self::MAX_TOTAL_PASSENGERS / 2;
    pub const MAX_WAITING_PASSENGERS: usize = 20;

    pub fn new(opts: GridOpts, python_agents: Vec<PythonAgentWrapper>) -> Self {
        assert_eq!(opts.agent_car_count, python_agents.len() as u32);

        // assign a traffic light to every road
        let traffic_lights = Self::generate_traffic_lights();
        let waiting_passengers = Self::generate_passengers(opts.initial_passenger_count);

        let mut this = Self {
            opts,

            // grid: HashMap::new(),
            cars: HashMap::default(),
            car_positions: HashMap::default(),

            waiting_passengers,
            waiting_passenger_positions: HashMap::default(),

            cars_to_spawn: Vec::new(),

            traffic_lights,

            ticks_passed: 0,

            tick_state: None,
            tick_events: Vec::new(),
        };

        // tmp: spawn X random cars
        // for _ in 0..1 {
        //     // let agent = RandomTurns {};
        //     // let agent = RandomDestination::default();
        //     // let agent = NearestPassenger::default();
        //     let agent = PythonAgent::default();
        //     let car = CarProps::new(agent, 3);
        //     this.add_car(car);
        // }

        // spawn required npc cars
        for _ in 0..opts.npc_car_count {
            let npc_props = CarProps::new(RandomTurns {}, Self::CAR_SPEED, BLUE);
            this.add_car(npc_props, None);
        }

        // spawn required agent cars
        let mut python_agents = python_agents.into_iter();
        let agent_car_colours = [RED, GREEN, ORANGE, PURPLE];
        for i in 0..opts.agent_car_count {
            // let agent = PythonAgent::new();
            // let car = CarProps::new(agent, 3);
            // this.add_car(car);

            let python_agent = PythonAgent::new(python_agents.next().unwrap());
            let colour = agent_car_colours[i as usize % agent_car_colours.len()];
            let agent_props = CarProps::new(python_agent, Self::CAR_SPEED, colour);
            this.add_car(agent_props, None);
        }

        this
    }

    fn generate_traffic_lights() -> HashMap<RoadSection, TrafficLight> {
        let mut traffic_lights = HashMap::default();

        for section in RoadSection::all() {
            let state = match section.direction.orientation() {
                Orientation::Horizontal => LightState::Green,
                Orientation::Vertical => LightState::Red,
            };
            let traffic_light = TrafficLight {
                toggle_every_ticks: Self::TRAFFIC_LIGHT_TOGGLE_TICKS,
                state,
                ticks_left: Self::TRAFFIC_LIGHT_TOGGLE_TICKS,
            };
            traffic_lights.insert(section, traffic_light);
        }

        traffic_lights
    }

    fn generate_passengers(count: u32) -> HashMap<PassengerId, Passenger> {
        let mut waiting_passengers = hashmap_with_capacity(Self::MAX_WAITING_PASSENGERS);

        let mut rng = rand::thread_rng();

        for _ in 0..count {
            let passenger = Passenger::random(&mut rng, 0);
            waiting_passengers.insert(passenger.id, passenger);
        }

        waiting_passengers
    }

    pub fn cars(&self) -> impl Iterator<Item = &Car> {
        self.cars.values()
    }

    pub fn car(&self, car: CarId) -> &Car {
        self.cars
            .get(&car)
            .unwrap_or_else(|| panic!("{car:?} not in grid.cars"))
    }

    pub fn car_mut(&mut self, car: CarId) -> &mut Car {
        self.cars
            .get_mut(&car)
            .unwrap_or_else(|| panic!("{car:?} not in grid.cars"))
    }

    pub fn car_position(&self, car: CarId) -> CarPosition {
        self.car(car).position
    }

    pub fn add_car(&mut self, props: CarProps, position: Option<CarPosition>) {
        self.cars_to_spawn.push((props, position));
    }

    pub fn has_car_at(&self, position: &CarPosition) -> bool {
        self.car_positions.contains_key(position)
    }

    pub fn traffic_light_at(&self, section: &RoadSection) -> &TrafficLight {
        &self.traffic_lights[section]
    }

    pub fn tick(&mut self) {
        if self.opts.verbose {
            print!("Tick: {} ", self.ticks_passed);
        }

        let tick_state = PyGridState::build(self);
        self.tick_state = Some(tick_state);
        self.tick_events.clear();

        self.tick_traffic_lights();
        self.tick_cars();
        self.tick_passengers();

        self.tick_state = None;
        self.ticks_passed += 1;

        let post_tick_state = PyGridState::build(self);
        self.send_transition_result(post_tick_state);

        if self.opts.verbose {
            println!();
        }
    }

    fn tick_traffic_lights(&mut self) {
        // up next
        for traffic_light in &mut self.traffic_lights.values_mut() {
            traffic_light.ticks_left -= 1;

            if traffic_light.ticks_left == 0 {
                traffic_light.state.toggle();
                traffic_light.ticks_left = traffic_light.toggle_every_ticks;
            }
        }
    }

    fn tick_cars(&mut self) {
        // move all the cars in the grid
        // this is done in 2 passes: first we calculate which cars want to move
        // where, while checking two cars don't want to move to the same place.
        // then we actually move them in phase 2.

        // to double check we don't lose cars
        let cars_count = self.cars.len();

        // list of before-and-after positions
        let mut cars_to_move = hashmap_with_capacity(self.cars.len());

        // set of after positions, to see if another car is already moving there
        let mut next_positions = hashmap_with_capacity(self.cars.len());

        // hashmap of positions, to easily check for car presence at coords
        let old_positions = self.cars().map(|car| car.position).collect::<HashSet<_>>();

        let car_ids = self.cars.keys().copied().collect::<Vec<_>>();
        for car_id in car_ids {
            // delete all "pick up" commands (they are per-tick)
            self.car_remove_pick_up_commands(car_id);
            // todo: reset all passenger.car_on_its_way to false

            let old_position = self.car_position(car_id);

            // by default, the car stays still
            let previous_car_at_pos = next_positions.insert(old_position, car_id);
            if let Some(prev_car_id) = previous_car_at_pos {
                panic!("{prev_car_id:?} tried to move to {old_position:?} even though {car_id:?} was already there");
            }

            // tick agent
            let decision = {
                // temporarily take agent out of car
                let mut agent = {
                    let car = self.car_mut(car_id);
                    let null_agent = Box::new(NullAgent {});
                    std::mem::replace(&mut car.props.agent, null_agent)
                };
                let decision = agent.get_turn(self, car_id);
                self.car_mut(car_id).props.agent = agent;

                decision
            };

            // if the car is at a red light, sit still
            if self.is_red_traffic_light(&old_position) {
                let car = self.car_mut(car_id);
                car.ticks_since_last_movement = 0;
                continue;
            }

            let next_position = self.car_next_position(car_id, decision);

            // if there is a car already there -> don't move there, cause that
            // car might not move (e.g. red light)
            // if there will be a car there next turn -> don't move either
            if old_positions.contains(&next_position) || next_positions.contains_key(&next_position)
            {
                continue;
            }

            // the car should move.
            cars_to_move.insert(old_position, next_position);
            next_positions.remove(&old_position);
            next_positions.insert(next_position, car_id);
        }

        // move the cars
        for car in self.cars.values_mut() {
            let Some(next_position) = cars_to_move.remove(&car.position) else {
                // car stays still
                car.ticks_since_last_movement += 1;
                continue;
            };

            assert_ne!(car.position, next_position);
            car.position = next_position;
            car.ticks_since_last_movement = 0;
        }

        self.car_positions = next_positions;

        let new_cars_count = self.cars_to_spawn.len();

        // spawn cars waiting to be spawned
        if !self.cars_to_spawn.is_empty() {
            let cars_to_spawn = std::mem::take(&mut self.cars_to_spawn);
            let mut rng = rand::thread_rng();

            for (props, position) in cars_to_spawn {
                let position = position.unwrap_or_else(|| self.random_empty_car_position(&mut rng));

                let car = Car::new(props, position);
                self.car_positions.insert(position, car.id());
                self.cars.insert(car.id(), car);
            }

            self.cars.shrink_to_fit();
        }

        // check we didn't lose any cars in the process
        assert_eq!(cars_count + new_cars_count, self.cars.len());
        assert_eq!(self.car_positions.len(), self.cars.len());
    }

    fn car_remove_pick_up_commands(&mut self, car_id: CarId) {
        let car = self.cars.get_mut(&car_id).unwrap();
        car.passengers.retain(|p| p.is_dropping_off());
    }

    fn car_next_position(&mut self, car_id: CarId, decision: CarDecision) -> CarPosition {
        // note: does NOT update car position! only calculates the next position

        let car = self.car(car_id);

        // cars can only move every "speed" ticks
        if car.ticks_since_last_movement < car.props.speed {
            return car.position;
        }

        // calculate next position, using decision if needed
        let old_position = car.position;
        let next_position = car.position.next();
        let next_position = match next_position {
            NextCarPosition::OnlyStraight(next) => next,
            NextCarPosition::MustChoose => old_position.take_decision(decision),
        };

        assert_ne!(old_position, next_position, "car turned but stayed still");
        next_position
    }

    fn tick_passengers(&mut self) {
        // spawn passengers
        let mut rng = rand::thread_rng();
        while rng.gen::<f32>() < self.opts.passenger_spawn_rate {
            let passenger = loop {
                let passenger = Passenger::random(&mut rng, self.ticks_passed);

                // make sure we don't spawn a passenger where there is one already
                let passenger_start_is_taken = self
                    .waiting_passenger_positions
                    .contains_key(&passenger.start);
                if !passenger_start_is_taken {
                    break passenger;
                }
            };

            let event = TickEvent::PassengerSpawned(passenger.id);
            self.tick_events.push(event);

            self.waiting_passenger_positions
                .insert(passenger.start, passenger.id);
            self.waiting_passengers.insert(passenger.id, passenger);
        }

        // pick up & drop off passengers
        for car in self.cars.values_mut() {
            let old_passengers = mem::take(&mut car.passengers);

            for passenger in old_passengers {
                match passenger {
                    CarPassenger::DroppingOff(passenger) => {
                        // === drop off passenger ===
                        let drop_off_here = passenger.destination == car.position;
                        if drop_off_here {
                            print!("Car dropped off passenger! ");
                            let event = TickEvent::PassengerDroppedOff(car.props.id, passenger);
                            self.tick_events.push(event);
                        } else {
                            // if we don't drop the passenger off, we keep them
                            car.passengers.push(CarPassenger::DroppingOff(passenger));
                        }
                    }

                    CarPassenger::PickingUp(passenger_id) => {
                        let passenger = self.waiting_passengers.get(&passenger_id);
                        let Some(passenger) = passenger else {
                            // this passenger just got picked up by another car
                            continue;
                        };

                        // if the car is right now next to that passenger
                        if car.position == passenger.start {
                            // pick them up:
                            // remove them from the sidewalk
                            let passenger = self.waiting_passengers.remove(&passenger_id).unwrap();
                            self.waiting_passenger_positions.remove(&passenger.start);

                            // create the event while we still own the passenger variable
                            let event = TickEvent::PassengerPickedUp(car.props.id, passenger.id);
                            self.tick_events.push(event);

                            // and finally put them into the car
                            let car_passenger = CarPassenger::DroppingOff(passenger);
                            car.passengers.push(car_passenger);

                            print!("Car picked up passenger! ");
                        }
                    }
                }
            }
        }
    }

    fn send_transition_result(&self, new_state: PyGridState) {
        for car in self.cars() {
            let Some(py_agent) = car.props.agent.as_py_agent() else {
                continue;
            };

            let new_state = new_state.with_pov(car);
            py_agent.end_of_tick(new_state);
        }
    }

    fn random_empty_car_position(&self, mut rng: impl Rng) -> CarPosition {
        for _ in 0..1000 {
            let position = CarPosition::random(&mut rng);
            if !self.has_car_at(&position) {
                return position;
            }
        }

        panic!("Grid is full!")
    }

    fn is_red_traffic_light(&self, car_pos: &CarPosition) -> bool {
        return car_pos.is_at_intersection()
            && self.traffic_lights[&car_pos.road_section].state == LightState::Red;
    }

    pub fn waiting_passengers(&self) -> impl Iterator<Item = &Passenger> {
        self.waiting_passengers.values()
    }

    pub fn unassigned_passengers(&self) -> Vec<&Passenger> {
        self.waiting_passengers
            .values()
            .filter(|p| !p.car_on_its_way)
            .collect()
    }

    pub fn assign_car_to_passenger(&mut self, car_id: CarId, passenger: PassengerId) {
        let passenger_id = self
            .waiting_passengers
            .get(&passenger)
            .expect("Car tried to assign to non-existent passenger")
            .id;

        let max_passengers_per_car = self.opts.passengers_per_car;
        let car = self.car_mut(car_id);
        if car.passengers.len() >= max_passengers_per_car {
            panic!("Car already has {} passengers", car.passengers.len());
        }

        car.passengers.push(CarPassenger::PickingUp(passenger_id));
    }

    pub fn get_idle_passenger(&self, passenger: PassengerId) -> Option<&Passenger> {
        self.waiting_passengers.get(&passenger)
    }

    pub fn py_state(&self, pov_car_id: CarId) -> PyGridState {
        let pov_car = self.car(pov_car_id);
        self.tick_state
            .as_ref()
            .expect("Grid::py_state() called outside of tick")
            .with_pov(pov_car)
    }
}
