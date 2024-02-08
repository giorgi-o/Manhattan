use std::collections::VecDeque;

use pathfinding::directed::astar::astar;

use super::{
    car::{CarDecision, CarPosition},
    grid::RoadSection,
};

pub struct Path {
    // first element is current section
    // last element is destination
    pub sections: VecDeque<RoadSection>,

    // need to store car position in section as well
    pub destination: CarPosition,
}

impl Path {
    pub fn find(from: CarPosition, to: CarPosition) -> Self {
        let first = from.road_section;
        let last = to.road_section;

        let heuristic = |section: &RoadSection| -> usize { section.manhattan_distance(last) };
        let successors = |section: &RoadSection| /*-> IntoIter<(RoadSection, usize)>*/ {
            let section = *section;
            let decisions = section.possible_decisions();
            decisions
                .into_iter()
                .map(move |d| section.take_decision(d).unwrap())
                .map(|s| (s, 1))
        };
        let reached_goal = |section: &RoadSection| -> bool { section == &last };

        let (sections, _cost) =
            astar(&first, successors, heuristic, reached_goal).expect("No path to destination");

        Self {
            sections: sections.into(),
            destination: to,
        }
    }

    pub fn pop_next_decision(&mut self) -> Option<CarDecision> {
        // returns None if we already arrived
        let current_section = self.sections.pop_front().unwrap();
        let next_section = self.sections.front()?;
        let decision = current_section.decision_to_go_to(self.sections[0]).unwrap();
        Some(decision)
    }
}
