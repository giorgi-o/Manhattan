use std::{cell::RefCell, collections::VecDeque, hash::Hash, ops::Deref};

use pathfinding::directed::astar::astar;

use super::{
    car::{Car, CarDecision, CarPosition},
    grid::{Grid, LightState, RoadSection},
};

#[derive(Clone, Debug)]
pub struct Path {
    // first element is current section
    // last element is destination
    pub sections: VecDeque<RoadSection>,

    // need to store car position in section as well
    pub destination: CarPosition,

    pub cost: usize,
}

impl Path {
    pub fn find<'g>(start: CarPosition, destination: CarPosition, speed: usize) -> Self {
        let graph = Graph {
            // grid,
            start,
            destination,
            // nodes: RefCell::default(),
            // nodes: vec![],
            speed,
        };

        let start = graph.start_node();

        // let node = |index: NodeIndex| &graph.nodes[index];

        let heuristic =
            |node: &Node| -> usize { node.section().manhattan_distance(destination.road_section) };
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

        let (sections, cost) =
            astar(&start, successors, heuristic, reached_goal).expect("No path to destination");

        let sections = sections.into_iter().map(|node| node.section()).collect();
        Self {
            sections,
            destination,
            cost,
        }
    }

    pub fn pop_next_decision(&mut self) -> Option<CarDecision> {
        // returns None if we already arrived
        let current_section = self.sections.pop_front().unwrap();
        let next_section = self.sections.front()?;
        let decision = current_section.decision_to_go_to(*next_section).unwrap();
        Some(decision)
    }

    pub fn next_decision(&self) -> Option<CarDecision> {
        // returns None if we already arrived
        let current_section = self.sections.front()?;
        let next_section = self.sections.get(1)?;
        current_section.decision_to_go_to(*next_section)
    }
}

type NodeIndex = usize;

struct Graph {
    start: CarPosition,
    destination: CarPosition,
    // nodes: RefCell<Vec<Node>>,
    // nodes: Vec<Node>,
    speed: usize,
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
        // fn successors(&'g self) -> impl Iterator<Item = (Node, usize)> {
        let possible_decisions = node.section().possible_decisions();

        let roads = possible_decisions
            .into_iter()
            .map(|d| node.section().take_decision(d).unwrap());
        let car_positions = roads.map(|r| CarPosition {
            road_section: r,
            position_in_section: 0,
        });
        let nodes = car_positions.map(|p| {
            let ticks_after_parent = self.ticks_to(node, p);
            Node {
                car_pos: p,
                ticks_after_parent,
                ticks_after_start: node.ticks_after_start + ticks_after_parent,
            }
        });
        let nodes_and_cost = nodes.map(|n| {
            let move_cost = n.ticks_after_parent;
            // let node_index = self.add_node(n);
            // (node_index, move_cost)
            (n, move_cost)
        });

        let mut successors = Vec::with_capacity(3);
        successors.extend(nodes_and_cost);
        successors
        // nodes_and_cost
    }

    // the cost to go here from here to a successor
    fn ticks_to(&self, node: &Node, to: CarPosition) -> usize {
        // count the ticks of:
        // 1. the car reaching the end of the section
        // 2. the traffic light turning green
        // 3. the car reaching the position in the next section

        // 1.
        let distance_from_road_end =
            node.section().direction.max_position_in_section() - node.car_pos.position_in_section;
        let car_speed = self.speed; // ticks per movement
        let time_to_road_end = distance_from_road_end * car_speed;

        // 2.
        // TMP: disabled because getting lifetimes to work with astar() is haaard
        // (feel free to try)
        // for now, just add 1 penalty if turning (not going straight) to
        // incentivise going from green light to green light
        let mut traffic_light_wait_time = 0;
        if node.car_pos.road_section.direction.orientation()
            != to.road_section.direction.orientation()
        {
            traffic_light_wait_time = 1;
        }

        // let traffic_light = self.grid.traffic_light_at(&node.section());
        // let traffic_light = traffic_light.time_travel(node.ticks_after_start + time_to_road_end);
        // let traffic_light_wait_time = if traffic_light.state == LightState::Green {
        //     0
        // } else {
        //     traffic_light.ticks_left
        // };

        // 3.
        let distance_from_road_start = to.position_in_section;
        let time_from_road_start = distance_from_road_start * car_speed;

        time_to_road_end + traffic_light_wait_time + time_from_road_start
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

#[derive(Clone)]
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
}

// astar() wants Node to be Eq + Hash + Clone
// can't #[derive] them cause Grid doesn't implement them

impl Hash for Node {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.car_pos.hash(state);
        self.ticks_after_parent.hash(state);
        self.ticks_after_start.hash(state);
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.car_pos == other.car_pos
            && self.ticks_after_parent == other.ticks_after_parent
            && self.ticks_after_start == other.ticks_after_start
    }
}

impl Eq for Node {}
