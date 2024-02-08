use rand::Rng;

use super::car::CarPosition;

pub struct Passenger {
    pub start: CarPosition,
    pub destination: CarPosition,
}

impl Passenger {
    pub fn random(mut rng: impl Rng) -> Self {
        Self {
            start: CarPosition::random(&mut rng),
            destination: CarPosition::random(rng),
        }
    }
}