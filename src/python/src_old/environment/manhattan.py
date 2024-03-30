import random
from typing import Any, TYPE_CHECKING

import torch

from environment.environment import (
    Action,
    ContinuousAction,
    ContinuousActionEnv,
    DiscreteAction,
    DiscreteActionEnv,
    Environment,
    State,
    Transition,
)
from network import NeuralNetwork

if TYPE_CHECKING:
    from dqn.dqn import DQN, DqnEpisode
else:
    DQN = object
    DqnEpisode = object


class ManhattanEnv(DiscreteActionEnv):
    def __init__(self, rust: Any, agent_cars: int, npc_cars: int, passenger_radius: int, render: bool):
        self.rust = rust

        self.agent_cars = agent_cars
        self.npc_cars = npc_cars
        self.passenger_radius = passenger_radius

        self.render = render

        self.current_grid_state: Any | None = None
        self.reward_since_last_timestep: int = 0
        self.last_action: Action | None = None
        self.last_action_grid_state: Any | None = None
        self.last_action_car: Any | None = None

        # will be set once self.reset() is called
        self.grid: Any

        # will be set by DQN
        self.dqn: DQN
        self.dqn_episode: DqnEpisode

    def won(self, transition: Transition) -> bool:
        return transition.new_state.terminal

    @property
    def observation_space_length(self) -> int:
        return self.passenger_radius * 2

    @property
    def action_list(self) -> list[DiscreteAction]:
        return [DiscreteAction(i) for i in range(self.passenger_radius)]

    def format_state(self, grid_state, car) -> State:
        # format: we look at 10 nearest passengers. for each one, store both
        # distance from car to passenger, then distance from passenger to
        # its destination.

        passengers: list[Any] = grid_state.waiting_passengers
        passengers.sort(key=lambda p: car.distance(p.start))

        passengers = passengers[: self.passenger_radius]

        state_tensor = torch.zeros(self.passenger_radius * 2).to(NeuralNetwork.device())
        for i, passenger in enumerate(passengers):
            index = i * 2

            state_tensor[index] = car.distance(passenger.start)
            state_tensor[index + 1] = car.distance_from(passenger.start, passenger.destination)

        terminal = len(passengers) == 0
        state = State(state_tensor, terminal)

        return state

    def reset(self):
        self.grid = self.rust.Grid(self.render)

        for _ in range(self.agent_cars):
            self.grid.add_agent_car()

        for _ in range(self.npc_cars):
            self.grid.add_npc_car()

    def agent_callback(self, car) -> int:
        assert self.dqn is not None

        # 1. build transition and send to dqn
        if self.last_action is not None:
            self.process_transition(car)

        # 2. get next action using neural network
        state = self.format_state(self.current_grid_state, car)
        action = self.dqn.get_action_using_epsilon_greedy(state)

        self.last_action = action
        self.last_action_grid_state = self.current_grid_state
        self.last_action_car = car

        passenger_index = action.action
        if passenger_index >= len(self.current_grid_state.waiting_passengers):
            passenger_index = 0
            self.reward_since_last_timestep -= 10000
        if passenger_index >= len(self.current_grid_state.waiting_passengers):
            raise ValueError(f"Invalid passenger index: {passenger_index}")
        passenger_id = self.current_grid_state.waiting_passengers[passenger_index].id

        return passenger_id

    # called then start of every tick
    def timestep_callback(self, new_grid_state):
        self.current_grid_state = new_grid_state
        self.reward_since_last_timestep -= len(new_grid_state.waiting_passengers)

        if len(new_grid_state.waiting_passengers) == 0:
            self.process_transition(self.last_action_car)

    def process_transition(self, car):
        assert self.last_action is not None

        transition = Transition(
            action=self.last_action,
            old_state=self.format_state(self.last_action_grid_state, car),
            new_state=self.format_state(self.current_grid_state, car),
            reward=self.reward_since_last_timestep,
            truncated=False,
        )
        self.dqn_episode.process_transition(transition)
        self.reward_since_last_timestep = 0

    def tick(self):
        self.grid.tick()

    def take_action(self, action: Action) -> Transition:
        raise NotImplementedError

    @property
    def current_state(self) -> State:
        raise NotImplementedError

    @property
    def needs_reset(self) -> bool:
        raise NotImplementedError

    @property
    def last_reward(self) -> float:
        raise NotImplementedError
