use std::sync::atomic::{AtomicUsize, Ordering};

use rand::Rng;

use super::car::CarPosition;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PassengerId(usize);

impl PassengerId {
    pub fn next() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        Self(NEXT_ID.fetch_add(1, Ordering::SeqCst))
    }
}

pub struct Passenger {
    pub id: PassengerId,
    pub start: CarPosition,
    pub destination: CarPosition,
    pub car_on_its_way: bool,
}

impl Passenger {
    pub fn random(mut rng: impl Rng) -> Self {
        Self {
            id: PassengerId::next(),
            start: CarPosition::random(&mut rng),
            destination: CarPosition::random(rng),
            car_on_its_way: false,
        }
    }
}
