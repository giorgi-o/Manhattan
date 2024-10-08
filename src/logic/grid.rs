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
    grid_util::{GridOpts, GridStats, LightState, PassengerEvent, TickEvent, TrafficLight},
    passenger::{Passenger, PassengerId},
    util::{hashmap_with_capacity, HashMap, HashSet, Orientation, RoadSection},
};

pub struct Grid {
    pub opts: GridOpts,

    pub cars: HashMap<CarId, Car>,
    car_positions: HashMap<CarPosition, CarId>,

    pub waiting_passengers: HashMap<PassengerId, Passenger>,
    waiting_passenger_positions: HashMap<CarPosition, PassengerId>,

    pub cars_to_spawn: Vec<CarToSpawn>,

    pub traffic_lights: HashMap<RoadSection, TrafficLight>,
    pub charging_stations: HashMap<ChargingStationId, ChargingStation>,

    pub ticks_passed: usize,
    pub stats: GridStats,

    pub tick_state: Option<PyGridState>,
    pub tick_events: Vec<TickEvent>,
}

impl Grid {
    pub const HORIZONTAL_ROADS: usize = 10;
    pub const VERTICAL_ROADS: usize = 15;
    pub const HORIZONTAL_SECTION_SLOTS: usize = 5;
    pub const VERTICAL_SECTION_SLOTS: usize = 5;

    pub const TRAFFIC_LIGHT_TOGGLE_TICKS: usize = 60; // 3s at 20TPS

    pub const CAR_SPEED: usize = 3;
    // pub const CAR_DISCHARGE_RATE: f32 = 0.002; // can go 500 ticks without charging

    // pub const MAX_TOTAL_PASSENGERS: usize = Self::HORIZONTAL_ROADS * Self::VERTICAL_ROADS;
    // pub const MAX_WAITING_PASSENGERS: usize = Self::MAX_TOTAL_PASSENGERS / 2;
    // pub const MAX_WAITING_PASSENGERS: usize = 20;

    pub fn new(opts: GridOpts, python_agents: Vec<PythonAgentWrapper>) -> Self {
        assert_eq!(opts.agent_car_count, python_agents.len() as u32);

        let traffic_lights = Self::generate_traffic_lights();
        let charging_stations = Self::generate_charging_stations(
            &opts.charging_stations,
            opts.charging_station_capacity,
        );

        let mut stats = GridStats::default();
        stats.ticks_with_n_passengers = vec![0; opts.passengers_per_car + 1];
        stats.ticks_picking_up_n_closest_passenger = vec![0; opts.passenger_radius + 1];
        stats.ticks_dropping_off_n_closest_passenger = vec![0; opts.passenger_radius + 1];

        let mut this = Self {
            opts: opts.clone(),

            cars: HashMap::default(),
            car_positions: HashMap::default(),

            waiting_passengers: HashMap::default(),
            waiting_passenger_positions: HashMap::default(),

            cars_to_spawn: Vec::new(),

            traffic_lights,
            charging_stations,

            ticks_passed: 0,
            stats,

            tick_state: None,
            tick_events: Vec::new(),
        };

        // spawn passengers
        this.generate_passengers();

        // spawn required npc cars
        for _ in 0..opts.npc_car_count {
            let npc_props = CarProps::new(RandomTurns {}, Self::CAR_SPEED, 0.0, BLUE);
            this.add_car(npc_props, None);
        }

        // spawn required agent cars
        let mut python_agents = python_agents.into_iter();
        let agent_car_colours = [RED, GREEN, ORANGE, PURPLE, PINK];
        for i in 0..opts.agent_car_count {
            let python_wrapper = python_agents.next().unwrap();
            let python_agent = PythonAgent::new(python_wrapper, opts.deterministic_mode);
            
            let colour = agent_car_colours[i as usize % agent_car_colours.len()];
            let agent_props =
                CarProps::new(python_agent, Self::CAR_SPEED, opts.discharge_rate, colour);
            
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

    fn generate_charging_stations(
        coords: &[CarPosition],
        capacity: usize,
    ) -> HashMap<ChargingStationId, ChargingStation> {
        coords
            .into_iter()
            .map(|coord| ChargingStation::new(Some(*coord), capacity, 0.01))
            .map(|cs| (cs.id, cs))
            .collect()
    }

    fn generate_passengers(&mut self) {
        assert_eq!(self.ticks_passed, 0);

        let count = self.opts.initial_passenger_count;
        self.waiting_passengers = hashmap_with_capacity(count as usize);

        let mut rng = rand::thread_rng();
        for _ in 0..count {
            let passenger = self.generate_passenger(&mut rng);
            self.waiting_passengers.insert(passenger.id, passenger);
        }
    }

    fn current_passenger_event(&self) -> Option<&PassengerEvent> {
        self.opts.passenger_events.iter().find(|event| {
            let (start_tick, end_tick) = &event.between_ticks;
            if start_tick.is_some_and(|start| self.ticks_passed < start)
                || end_tick.is_some_and(|end| self.ticks_passed > end)
            {
                return false;
            }
            true
        })
    }

    fn generate_passenger(&self, mut rng: impl Rng) -> Passenger {
        let event = self.current_passenger_event();

        for _ in 0..1000 {
            let passenger = match event {
                Some(event) => Passenger::random_in_event(&mut rng, self.ticks_passed, event),
                None => Passenger::random(&mut rng, self.ticks_passed),
            };

            // if there's a passenger event currently, respect it
            if let Some(event) = event {
                let (sx1, sy1, sx2, sy2) = event.start_area;
                let (sx, sy) = passenger.start.road_section.checkerboard_coords();
                if sx < sx1 || sx > sx2 || sy < sy1 || sy > sy2 {
                    panic!("generated passenger outside of passenger event area");
                }

                let (dx1, dy1, dx2, dy2) = event.destination_area;
                let (dx, dy) = passenger.destination.road_section.checkerboard_coords();
                if dx < dx1 || dx > dx2 || dy < dy1 || dy > dy2 {
                    panic!("generated passenger outside of passenger event area");
                }
            }

            // make sure we don't spawn a passenger where there is one already
            let passenger_start_is_taken = self
                .waiting_passenger_positions
                .contains_key(&passenger.start);
            // also don't spawn one if there is a charging station there
            let passenger_start_is_charging_station =
                self.charging_station_entrance_at(passenger.start).is_some();

            if passenger_start_is_taken || passenger_start_is_charging_station {
                continue; // try again
            }

            return passenger;
        }

        panic!("could not generate passenger, is the event area too small?");
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

    pub fn unspawned_car(&self, car_id: CarId) -> &CarToSpawn {
        self.cars_to_spawn
            .iter()
            .find(|cs| cs.props.id == car_id)
            .unwrap()
    }

    pub fn add_car(&mut self, props: CarProps, position: Option<CarPosition>) {
        let car_to_spawn = CarToSpawn {
            props,
            position,
            out_of_battery: None,
        };
        self.cars_to_spawn.push(car_to_spawn);
    }

    pub fn has_car_at(&self, position: &CarPosition) -> bool {
        self.car_positions.contains_key(position)
    }

    pub fn traffic_light_at(&self, section: &RoadSection) -> &TrafficLight {
        &self.traffic_lights[section]
    }

    pub fn charging_station_entrance_at(&self, pos: CarPosition) -> Option<&ChargingStation> {
        let id1 = ChargingStationId::from(pos);
        let id2 = ChargingStationId::from(pos.other_side_of_road());

        self.charging_stations
            .get(&id1)
            .or_else(|| self.charging_stations.get(&id2))
    }

    pub fn charging_station_at_mut(&mut self, pos: CarPosition) -> Option<&mut ChargingStation> {
        let id = ChargingStationId::from(pos);
        self.charging_stations.get_mut(&id)
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
        self.stats.ticks += 1;

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
        let new_cars_count = self.cars_to_spawn.len();

        // map of before-and-after positions
        let mut cars_to_move = hashmap_with_capacity(self.cars.len());

        // map of after positions, to see if another car is already moving there
        let mut next_positions = hashmap_with_capacity(self.cars.len());

        // set of before positions, to easily check for car presence at coords
        let old_positions = self.cars().map(|car| car.position).collect::<HashSet<_>>();

        let car_ids = self.cars.keys().copied().collect::<Vec<_>>();
        for car_id in car_ids {
            // delete all "pick up" commands (they are per-tick)
            self.car_remove_pick_up_commands(car_id);
            for passenger in self.waiting_passengers.values_mut() {
                passenger.car_on_its_way = false;
            }

            let old_position = self.car_position(car_id);

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

            let next_position = 'next_pos: {
                // if the car is at a red light, sit still
                if self.is_red_traffic_light(&old_position) {
                    break 'next_pos old_position;
                }

                let neighbour_cs = match old_position.in_charging_station {
                    Some(cs_id) => Some(self.charging_stations.get(&cs_id).unwrap()),
                    None => self.charging_station_entrance_at(old_position),
                };

                let car = self.car(car_id);
                let next_position = car.next_position(decision, neighbour_cs);

                let car_passenger_count = car.passengers.len();
                self.stats.ticks_with_n_passengers[car_passenger_count] += 1;

                // if there is a car already there -> don't move there, cause that
                // car might not move (e.g. red light)
                // if there will be a car there next turn -> don't move either
                if old_positions.contains(&next_position)
                    || next_positions.contains_key(&next_position)
                {
                    break 'next_pos old_position;
                }

                // the car should move.
                break 'next_pos next_position;
            };

            // add the car movement to the list
            cars_to_move.insert(old_position, next_position);

            let prev_car = next_positions.insert(next_position, car_id);
            if let Some(prev_car_id) = prev_car {
                panic!("{car_id:?} tried to move to {old_position:?} even though {prev_car_id:?} was already there");
            }
        }

        let mut cars_out_of_battery = vec![];

        // move the cars
        for car in self.cars.values_mut() {
            car.ticks_since_out_of_battery = car.ticks_since_out_of_battery.saturating_add(1);

            let Some(next_position) = cars_to_move.remove(&car.position) else {
                panic!("{:?} was not in cars_to_move (no next position)", car.id());
            };

            // if the car is at a charging station, charge its battery
            if let Some(cs_id) = car.position.in_charging_station {
                let cs = self.charging_stations.get(&cs_id).unwrap();
                assert!(cs.cars.contains(&car.id()), "{:?} not in cs.cars", car.id());

                car.battery.charging(cs);
                car.active_action = None;
            }

            if car.position != next_position {
                // car moves

                // tick car battery
                if !car.props.agent.is_npc() {
                    car.battery.discharge(car.props.discharge_rate);
                    // car.battery.discharge(0.01);
                }

                if car.battery.is_empty()
                    && !car.props.agent.is_npc()
                    && !car.position.is_at_charging_station()
                {
                    // car ran out of battery
                    cars_out_of_battery.push(car.id());
                    next_positions.remove(&next_position);
                    // can't do any more processing here because can't edit
                    // self.cars while iterating over it
                    // note: if the car was in the charging station with a
                    // negative battery and tried to leave, let it. it will
                    // be punished and brought right back here next tick >:)

                    self.stats.out_of_battery += 1;
                } else {
                    let old_position = car.position;

                    // move the car
                    car.position = next_position;
                    car.ticks_until_next_movement = car.props.speed;

                    // if the car entered/left charging station, tell the cs
                    // note: we assume a car can't teleport from one cs to another
                    if old_position.in_charging_station.is_some()
                        && next_position.in_charging_station.is_some()
                    {
                        // car stays in same cs, do nothing
                    } else if let Some(cs_id) = old_position.in_charging_station {
                        let cs = self.charging_stations.get_mut(&cs_id).unwrap();
                        let car_index_in_cs = cs.cars.iter().position(|c| *c == car.id()).unwrap_or_else( ||
                            panic!("car {:?} says it's in charging station, but charging station doesn't know about car",
                                car.id())
                        );

                        cs.cars.swap_remove(car_index_in_cs);
                    } else if let Some(cs_id) = next_position.in_charging_station {
                        let cs = self.charging_stations.get_mut(&cs_id).unwrap();

                        assert!(cs.has_space());
                        cs.cars.push(car.id());

                        self.stats.enter_charging_stations += 1;
                    }
                }
            } else {
                // car stays still
                // this could be either because it should only move forward every
                // "speed" ticks, or because there's something in front (traffic light
                // or other car)
                car.ticks_until_next_movement = car.ticks_until_next_movement.saturating_sub(1);
            }

            // assert_ne!(car.position, next_position);
            // car.position = next_position;
            // car.ticks_since_last_movement = 0;
        }

        // process "out of battery" cars
        for car_id in cars_out_of_battery {
            // remove car from grid
            let mut car = self.cars.remove(&car_id).unwrap();
            car.ticks_since_out_of_battery = 0;

            // assert the car wasn't at a charging station
            assert!(!car.position.is_at_charging_station());

            // add event
            let event = TickEvent::CarOutOfBattery(car.props.id, car.position);
            self.tick_events.push(event);

            // add car to list of cars to spawn
            let car_to_spawn = CarToSpawn {
                props: car.props,
                position: None,
                out_of_battery: Some((car.position, car.passengers)),
            };
            self.cars_to_spawn.push(car_to_spawn);
        }

        self.car_positions = next_positions;

        // spawn cars waiting to be spawned
        if !self.cars_to_spawn.is_empty() {
            let cars_to_spawn = std::mem::take(&mut self.cars_to_spawn);
            let mut rng = rand::thread_rng();

            for mut car_to_spawn in cars_to_spawn {
                if let Some((out_of_battery_position, passengers)) = car_to_spawn.out_of_battery {
                    // car ran out of battery on the road. look for the nearest
                    // charging station with space, and spawn it there.
                    let closest_charging_station = self
                        .charging_stations
                        .values()
                        .filter(|cs| cs.has_space())
                        .min_by_key(|cs| out_of_battery_position.distance_to(cs.entrance));

                    let Some(closest_charging_station) = closest_charging_station else {
                        // no charging station has space. just put the car back
                        // into self.cars_to_spawn for next tick

                        // put this back (we partially moved it in let-some)
                        car_to_spawn.out_of_battery = Some((out_of_battery_position, passengers));

                        self.cars_to_spawn.push(car_to_spawn);
                        continue;
                    };

                    // spawn the car at the charging station
                    let position = CarPosition::at_charging_station(closest_charging_station);
                    let mut car = Car::new(car_to_spawn.props, position, -1.0);
                    car.passengers = passengers;

                    // self.car_positions.insert(position, car.id());
                    // self.cars.insert(car.id(), car);
                    self.spawn_car(car);
                    continue;
                }

                let pos_is_taken = |pos: &_| self.car_positions.contains_key(pos);
                let car_position = car_to_spawn.position(&mut rng, pos_is_taken);
                let initial_battery = match self.opts.discharge_rate == 0.0 {
                    true => 1.0, // if battery never discharges, always have full battery
                    false => 0.2,
                };
                let car = Car::new(car_to_spawn.props, car_position, initial_battery);

                // self.car_positions.insert(car_position, car.id());
                // self.cars.insert(car.id(), car);
                self.spawn_car(car);
            }

            // self.cars.shrink_to_fit();
        }

        // check we didn't lose any cars in the process
        assert_eq!(
            cars_count + new_cars_count,
            self.cars.len() + self.cars_to_spawn.len()
        );
        assert_eq!(self.car_positions.len(), self.cars.len(),
        "car_positions.len() != self.cars.len() | cars_count={cars_count}, new_cars_count={new_cars_count}");

        // check all charging_station.cars are in sync with self.cars
        for cs in self.charging_stations.values() {
            for car_id in &cs.cars {
                assert!(self.cars.contains_key(car_id));
            }
        }
        for car in self.cars.values() {
            if car.position.is_at_charging_station() {
                let cs_id = car.position.in_charging_station.unwrap();
                let cs = self.charging_stations.get(&cs_id).unwrap();

                assert!(cs.cars.contains(&car.id()));
            }
        }
    }

    fn car_remove_pick_up_commands(&mut self, car_id: CarId) {
        let car = self.cars.get_mut(&car_id).unwrap();
        car.passengers.retain_mut(|p| {
            match p {
                CarPassenger::PickingUp(_) => false, // discard picking up
                CarPassenger::DroppingOff(p) => {
                    p.car_on_its_way = false; // all pickup commands get reset between ticks
                    true
                }
            }
        });
    }

    fn spawn_car(&mut self, car: Car) {
        let prev_car = self.car_positions.insert(car.position, car.id());
        if let Some(prev_car) = prev_car {
            panic!(
                "{id:?} tried to spawn at {pos:?} even though {prev_id:?} was already there",
                id = car.id(),
                pos = car.position,
                prev_id = prev_car
            );
        }

        // if it's being spawned in a charging station,
        // tell the cs it has a car now
        if let Some(cs_id) = car.position.in_charging_station {
            let cs = self.charging_stations.get_mut(&cs_id).unwrap();
            cs.cars.push(car.id());
        }

        let dupe_car = self.cars.insert(car.id(), car);
        if let Some(dupe_car) = dupe_car {
            panic!(
                "Tried to add {id:?} to grid.cars but it was already there",
                id = dupe_car.id()
            );
        }
    }

    fn tick_passengers(&mut self) {
        // spawn passengers
        let passenger_spawn_rate_this_tick = self
            .current_passenger_event()
            .and_then(|e| e.spawn_rate)
            .unwrap_or(self.opts.passenger_spawn_rate);
        let mut rng = rand::thread_rng();
        while self.waiting_passengers.len() < self.opts.max_passengers
            && rng.gen::<f32>() < passenger_spawn_rate_this_tick
        {
            let passenger = self.generate_passenger(&mut rng);

            let event = TickEvent::PassengerSpawned(passenger.id);
            self.tick_events.push(event);

            self.waiting_passenger_positions
                .insert(passenger.start, passenger.id);
            self.waiting_passengers.insert(passenger.id, passenger);

            self.stats.passenger_spawns += 1;
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
                            // print!("Car dropped off passenger! ");
                            let event = TickEvent::PassengerDroppedOff(car.props.id, passenger);
                            self.tick_events.push(event);
                            self.stats.passenger_dropoffs += 1;
                            car.active_action = None;
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

                            // print!("Car picked up passenger! ");
                            // std::io::stdout().flush().unwrap();
                            self.stats.passenger_pickups += 1;
                            car.active_action = None;
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
