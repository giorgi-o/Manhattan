use std::sync::atomic::{AtomicUsize, Ordering};

use pyo3::prelude::*;

use super::{
    car::{CarId, CarPosition},
    util::RoadSection,
};

pub struct BatteryPercent(f32);

impl BatteryPercent {
    const MIN_BATTERY: f32 = -1.0;

    pub fn new(percent: f32) -> Self {
        assert!(percent >= Self::MIN_BATTERY && percent <= 1.0);
        Self(percent)
    }

    pub fn get(&self) -> f32 {
        self.0
    }

    pub fn charging(&mut self, station: &ChargingStation) {
        let new_percent = self.0 + station.charging_speed.get();
        let new_percent = new_percent.min(1.0);

        self.0 = new_percent;
    }

    pub fn discharge(&mut self, rate: f32) {
        let new_percent = self.0 - rate;
        let new_percent = new_percent.max(0.0);

        self.0 = new_percent;
    }

    pub fn is_empty(&self) -> bool {
        self.0 <= 0.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[pyclass]
pub struct ChargingStationId {
    // a charging station is defined by where it is on the map
    road_section: RoadSection,
    position_in_section: usize,
    // we can't store a CarPosition here because CarPosition can store a
    // ChargingStationId -> recursive struct pair
    // and we can't use a Box because either we or CarPosition would have
    // to not be Copy
    // so we just store everything required to make our own RoadSection
    // when needed
}

impl ChargingStationId {
    pub fn from(entrance: CarPosition) -> Self {
        assert!(!entrance.is_at_charging_station());
        Self {
            road_section: entrance.road_section,
            position_in_section: entrance.position_in_section,
        }
    }

    pub fn entrance(&self) -> CarPosition {
        CarPosition {
            road_section: self.road_section,
            position_in_section: self.position_in_section,
            in_charging_station: None,
        }
    }
}

impl std::fmt::Debug for ChargingStationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (x, y) = self.road_section.checkerboard_coords();
        write!(f, "CSid({:.0}{:.0}{})", x, y, self.position_in_section)
    }
}

pub struct ChargingStation {
    pub id: ChargingStationId,
    pub entrance: CarPosition,
    pub capacity: usize,
    pub charging_speed: BatteryPercent, // per tick

    pub cars: Vec<CarId>,
}

impl ChargingStation {
    pub fn new(
        entrance: Option<CarPosition>, // None for random
        capacity: usize,
        charging_speed: f32,
    ) -> Self {
        let entrance = entrance.unwrap_or_else(|| CarPosition::random(rand::thread_rng()));

        Self {
            id: ChargingStationId::from(entrance),
            entrance,
            capacity,
            charging_speed: BatteryPercent::new(charging_speed),
            cars: vec![],
        }
    }

    pub fn has_space(&self) -> bool {
        self.cars.len() < self.capacity
    }
}
