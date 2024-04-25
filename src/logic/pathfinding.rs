use std::{collections::VecDeque, hash::Hash};

use pathfinding::directed::astar::astar;

use super::{
    car::{CarDecision, CarPosition},
    car_agent::AgentAction,
    util::RoadSection,
};

#[derive(Clone, Debug)]
pub struct Path {
    // first element is current section
    // last element is destination
    pub sections: VecDeque<RoadSection>,

    // need to store car position in section as well
    pub destination: CarPosition,

    pub cost: usize,

    pub action: Option<AgentAction>,
}

impl Path {
    pub fn find(start: CarPosition, destination: CarPosition) -> Self {
        if start.road_section == destination.road_section
            && start.position_in_section <= destination.position_in_section
        {
            return Self {
                sections: VecDeque::from([start.road_section]),
                destination,
                cost: (destination.position_in_section - start.position_in_section),
                action: None,
            };
        }

        let graph = Graph {
            start,
            destination,
        };

        let start = graph.start_node();

        // let node = |index: NodeIndex| &graph.nodes[index];

        let heuristic = |node: &Node| -> usize { node.manhattan_distance(destination) };
        let successors = |node: &Node| {
            // let node = node(*i);
            // let successors = graph.successors(node);
            // drop(node);
            // successors
            graph.successors(node)

            // successors
            //     .into_iter()
            //     .map(|(n, c)| {
            //         let i = graph.add_node(n);
            //         (i, c)
            //     })
            //     .collect::<Vec<_>>()

            // todo!()
        };
        let reached_goal = |node: &Node| -> bool { node.is_end_node(destination) };

        let (sections, mut cost) =
            astar(&start, successors, heuristic, reached_goal).expect("No path to destination");
        cost += destination.position_in_section;

        let sections = sections.into_iter().map(|node| node.section()).collect();
        Self {
            sections,
            destination,
            cost,
            action: None,
        }
    }

    pub fn distance(start: CarPosition, end: CarPosition, speed: usize) -> usize {
        let path = Self::find(start, end);
        path.cost
    }

    pub fn next_decision(&self) -> Option<CarDecision> {
        // returns None if we already arrived
        let current_section = self.sections.front()?;
        let next_section = self.sections.get(1)?;
        current_section.decision_to_go_to(*next_section)
    }
}

struct Graph {
    start: CarPosition,
    destination: CarPosition,
}

impl Graph {
    fn start_node(&self) -> Node {
        Node {
            car_pos: self.start,
            ticks_after_start: 0,
            ticks_after_parent: 0,
        }
    }

    fn successors(&self, node: &Node) -> Vec<(Node, usize)> {
        
        let possible_decisions = node.car_pos.possible_decisions();

        let roads = possible_decisions
            .into_iter()
            .filter(|d| *d != CarDecision::ChargeBattery)
            .filter_map(|d| node.section().take_decision(d));
        let car_positions = roads.map(|r| CarPosition {
            road_section: r,
            position_in_section: 0,
            in_charging_station: None,
        });
        let nodes = car_positions.map(|p| {
            let ticks_after_parent = self.cost_to(node, p);
            Node {
                car_pos: p,
                ticks_after_parent,
                ticks_after_start: node.ticks_after_start + ticks_after_parent,
            }
        });
        let nodes_and_cost = nodes.map(|n| {
            let move_cost = n.ticks_after_parent;
            (n, move_cost)
        });

        let mut successors = Vec::with_capacity(3);
        successors.extend(nodes_and_cost);
        successors
        // nodes_and_cost
    }

    // the cost to go here from here to a successor
    fn cost_to(&self, node: &Node, to: CarPosition) -> usize {
        assert_ne!(node.car_pos.road_section, to.road_section);

        // count the ticks of:
        // 1. the car reaching the start of the next section
        // 3. the car reaching the position in the next section

        // 1.
        let distance_from_road_end =
            node.section().direction.max_position_in_section() + 1 - node.car_pos.position_in_section;
        let time_to_road_end = distance_from_road_end;

        // 3.
        let distance_from_road_start = to.position_in_section;
        let time_from_road_start = distance_from_road_start;

        time_to_road_end + time_from_road_start
    }

    // fn add_node(&self, node: Node) -> NodeIndex {
    //     let mut nodes = self.nodes.borrow_mut();
    //     nodes.push(node);
    //     nodes.len() - 1
    // }

    // fn add_node(&self, node: Node) -> NodeIndex {
    //     self.nodes.push(node);
    //     self.nodes.len() - 1
    // }

    // // fn get_node(&self, index: NodeIndex) -> &'g Node {
    // fn get_node(&'g self, index: NodeIndex) -> impl Deref<Target = Node> + 'g {
    //     struct NodeHolder<'g> {
    //         vec_ref: std::cell::Ref<'g, Vec<Node>>,
    //         index: NodeIndex,
    //     }

    //     impl<'g> Deref for NodeHolder<'g> {
    //         type Target = Node;

    //         fn deref(&self) -> &Self::Target {
    //             &self.vec_ref[self.index]
    //         }
    //     }

    //     let vec_ref = self.nodes.borrow();
    //     NodeHolder { vec_ref, index }
    // }
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct Node {
    car_pos: CarPosition,

    ticks_after_parent: usize,
    ticks_after_start: usize,
}

impl Node {
    fn section(&self) -> RoadSection {
        self.car_pos.road_section
    }

    fn is_end_node(&self, destination: CarPosition) -> bool {
        if destination.road_section != self.car_pos.road_section {
            return false;
        }

        self.car_pos.position_in_section <= destination.position_in_section
    }

    fn manhattan_distance(&self, destination: CarPosition) -> usize {
        self.car_pos.manhattan_distance(destination)
    }
}
