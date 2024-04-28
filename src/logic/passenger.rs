use std::sync::atomic::{AtomicUsize, Ordering};

use macroquad::color::{Color, ORANGE, RED};
use rand::Rng;

use super::{car::CarPosition, grid_util::PassengerEvent};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PassengerId(usize);

impl PassengerId {
    pub fn next() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        Self(NEXT_ID.fetch_add(1, Ordering::SeqCst))
    }
}

#[derive(Debug)]
pub struct Passenger {
    pub id: PassengerId,
    pub start: CarPosition,
    pub destination: CarPosition,
    pub car_on_its_way: bool,
    pub colour: Color,
    pub start_tick: usize,
}

impl Passenger {
    pub fn random(mut rng: impl Rng, current_tick: usize) -> Self {
        Self {
            id: PassengerId::next(),
            start: CarPosition::random(&mut rng),
            destination: CarPosition::random(rng),
            car_on_its_way: false,
            colour: ORANGE,
            start_tick: current_tick,
        }
    }

    pub fn random_in_event(mut rng: impl Rng, current_tick: usize, event: &PassengerEvent) -> Self {
        let mut start = None;
        for _ in 0..1000 {
            let start_pos = CarPosition::random_in_area(&mut rng, event.start_area);
            let (sx, sy) = start_pos.road_section.checkerboard_coords();
            let (sx1, sy1, sx2, sy2) = event.start_area;
            if sx >= sx1 && sx <= sx2 && sy >= sy1 && sy <= sy2 {
                start = Some(start_pos);
                break;
            }
        }
        let start = start.expect("Could not find a random start position in event");

        let mut destination = None;
        loop {
            let destination_pos = CarPosition::random_in_area(&mut rng, event.destination_area);
            let (dx, dy) = destination_pos.road_section.checkerboard_coords();
            let (dx1, dy1, dx2, dy2) = event.destination_area;
            if dx >= dx1 && dx <= dx2 && dy >= dy1 && dy <= dy2 {
                destination = Some(destination_pos);
                break;
            }
        }
        let destination =
            destination.expect("Could not find a random destination position in event");

        Self {
            id: PassengerId::next(),
            start,
            destination,
            car_on_its_way: false,
            colour: RED,
            start_tick: current_tick,
        }
    }
}
