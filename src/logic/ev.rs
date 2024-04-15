use std::sync::atomic::{AtomicUsize, Ordering};

use pyo3::prelude::*;

use super::car::{CarId, CarPosition};

pub struct BatteryPercent(f32);

impl BatteryPercent {
    pub fn new(percent: f32) -> Self {
        Self(percent)
    }

    pub fn get(&self) -> f32 {
        self.0
    }

    pub fn charging(self, station: &ChargingStation) -> Self {
        let new_percent = self.0 + station.charging_speed.get();
        let new_percent = new_percent.min(1.0);

        Self(new_percent)
    }

    pub fn discharging(self, rate: f32) -> Self {
        let new_percent = self.0 - rate;
        let new_percent = new_percent.max(0.0);

        Self(new_percent)
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[pyclass]
pub struct ChargingStationId(usize);

impl ChargingStationId {
    pub fn next() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        Self(NEXT_ID.fetch_add(1, Ordering::SeqCst))
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
        Self {
            id: ChargingStationId::next(),
            entrance: entrance.unwrap_or_else(|| CarPosition::random(rand::thread_rng())),
            capacity,
            charging_speed: BatteryPercent::new(charging_speed),
            cars: vec![],
        }
    }
}
