import random
from dataclasses import dataclass
from typing import NewType, Any

import gymnasium as gym
from gymnasium import spaces
import numpy as np
from stable_baselines3.common.vec_env import VecEnv

RustModule = NewType("RustModule", Any)  # type: ignore
GridEnv = NewType("GridEnv", Any)  # type: ignore
GridState = NewType("GridState", Any)  # type: ignore
GridOpts = NewType("GridOpts", Any)  # type: ignore


@dataclass
class EnvOpts:
    passenger_radius: int
    car_radius: int
    passengers_per_car: int  # todo move to rust
    render: bool


class GridVecEnv(VecEnv):
    def __init__(
        self,
        rust: RustModule,
        grid_opts: GridOpts,
        env_opts: EnvOpts,
    ):
        self.rust = rust
        self.grid_opts = grid_opts
        self.env_opts = env_opts

        self.passenger_radius = env_opts.passenger_radius
        self.car_radius = env_opts.car_radius
        self.passengers_per_car = env_opts.passengers_per_car

        self.width, self.height = rust.grid_dimensions()

        self.render_mode = None  # we handle rendering ourselves

        num_envs = grid_opts.agent_car_count
        observation_space = self._observation_space()
        action_space = self._action_space()
        super().__init__(num_envs, observation_space, action_space)

        self.callbacks = [CarCallback(self) for _ in range(num_envs)]
        self.reset()
        self.env: GridEnv

    def step(self, actions: np.ndarray) -> tuple[tuple[np.ndarray, ...], np.ndarray, np.ndarray, list[dict]]:
        transitions: list[tuple | None] = [None] * self.num_envs

        for i in range(self.num_envs):
            cb = self.callbacks[i]

            cb.next_action = actions[i]
            cb.transitions_arr = transitions
            cb.transitions_idx = i

        self.env.tick()

        observations = []
        rewards = []
        done = np.array([False] * self.num_envs)
        information = [{}] * self.num_envs

        for transition in transitions:
            assert transition is not None
            old_state, action, new_state, reward = transition

            observations.append(new_state)
            rewards.append(reward)

        return tuple(observations), np.array(rewards), done, information

    def reset(self) -> tuple[np.ndarray, ...]:
        self.env = self.rust.PyGridEnv(self.callbacks, self.grid_opts, self.env_opts.render)

        Dir = self.rust.Direction
        random_direction = lambda: random.choice([Dir.Up, Dir.Down, Dir.Left, Dir.Right])

        head_towards = lambda d: self.rust.PyAction.head_towards(d, raw=None)
        actions = np.array([head_towards(random_direction()) for _ in range(self.num_envs)])

        observations, _, _, _ = self.step(actions)
        return observations

    def step_async(self, actions: np.ndarray) -> None:
        raise NotImplementedError

    def step_wait(self):
        raise NotImplementedError

    def close(self) -> None:
        raise NotImplementedError

    def get_attr(self, attr_name: str, indices=None) -> list[Any]:
        match attr_name:
            case "render_mode":
                return [None] * self.num_envs
            case _:
                raise NotImplementedError

    def set_attr(self, attr_name: str, value: Any, indices=None) -> None:
        raise NotImplementedError

    def env_method(self, method_name: str, *method_args, indices=None, **method_kwargs) -> list[Any]:
        raise NotImplementedError

    def env_is_wrapped(self, wrapper_class: type, indices=None) -> list[bool]:
        raise NotImplementedError

    def _observation_space(self) -> gym.Space:
        direction_spc = spaces.Discrete(4)  # N/S/E/W
        coords_ospc = spaces.Dict(
            {
                "x": spaces.Discrete(self.width),
                "y": spaces.Discrete(self.height),
                "direction": direction_spc,
            }
        )

        car_passenger_ospc = spaces.Dict(
            {
                "present": spaces.Discrete(2),
                "destination": coords_ospc,
                "distance_to_dest": spaces.Discrete(100),
                "time_since_request": spaces.Discrete(300),
            }
        )
        car_passengers_ospc = spaces.Tuple((car_passenger_ospc,) * self.passengers_per_car)

        idle_passenger_ospc = spaces.Dict(
            {
                "present": spaces.Discrete(2),
                "pos": coords_ospc,
                "destination": coords_ospc,
                "distance_to_dest": spaces.Discrete(100),
                "time_since_request": spaces.Discrete(300),
            }
        )
        idle_passengers_ospc = spaces.Tuple((idle_passenger_ospc,) * self.passenger_radius)

        car_ospc = spaces.Dict(
            {
                "pos": coords_ospc,  # pos
                "passengers": car_passengers_ospc,
            }
        )
        cars_ospc = spaces.Tuple((car_ospc,) * (self.car_radius))

        return spaces.Dict(
            {
                "pov_car": car_ospc,
                "other_cars": cars_ospc,
                "idle_passengers": idle_passengers_ospc,
            }
        )

    def _action_space(self) -> gym.Space:
        # can pick passenger to drop off, pick up, or to head N/S/E/W
        self.action_count = self.passengers_per_car + self.passenger_radius + 4
        return spaces.Discrete(self.action_count)

    def _parse_direction(self, direction) -> int:
        Direction = self.rust.Direction
        return {Direction.Up: 0, Direction.Right: 1, Direction.Down: 2, Direction.Left: 3}[direction]

    def _parse_coords(self, coords) -> dict[str, int]:
        Direction = self.rust.Direction

        horizontal = coords.direction in [Direction.Right, Direction.Left]
        if horizontal:
            parsed_coords = {"x": coords.section, "y": coords.road}
        else:
            parsed_coords = {"x": coords.road, "y": coords.section}

        parsed_coords["direction"] = self._parse_direction(coords.direction)
        return parsed_coords

    def _null_coords(self) -> dict[str, int]:
        return {"x": 0, "y": 0, "direction": 0}

    def _parse_car_passengers(self, car):
        passengers = []

        for passenger in car.passengers:
            passengers.append(
                {
                    "present": 1,
                    "destination": self._parse_coords(passenger.destination),
                    "distance_to_dest": passenger.distance_to_destination,
                    "time_since_request": passenger.ticks_since_request,
                }
            )

        while len(passengers) < self.passengers_per_car:
            passengers.append(
                {
                    "present": 0,
                    "destination": self._null_coords(),
                    "distance_to_dest": 0,
                    "time_since_request": 0,
                }
            )

        return tuple(passengers)

    def _parse_idle_passengers(self, idle_passengers):
        parsed_idle_passengers = []

        for passenger in idle_passengers:
            parsed_idle_passengers.append(
                {
                    "present": 1,
                    "pos": self._parse_coords(passenger.pos),
                    "destination": self._parse_coords(passenger.destination),
                    "distance_to_dest": passenger.distance_to_destination,
                    "time_since_request": passenger.ticks_since_request,
                }
            )

            if len(idle_passengers) == self.passenger_radius:
                break

        while len(parsed_idle_passengers) < self.passenger_radius:
            parsed_idle_passengers.append(
                {
                    "present": 0,
                    "pos": self._null_coords(),
                    "destination": self._null_coords(),
                    "distance_to_dest": 0,
                    "time_since_request": 0,
                }
            )

        return tuple(parsed_idle_passengers)

    def _parse_car(self, car):
        return {
            "pos": self._parse_coords(car.position),
            "passengers": self._parse_car_passengers(car),
        }

    def _parse_cars(self, cars):
        cars = []

        for car in cars:
            if car.ty != self.rust.PyCarType.Agent:
                continue

            cars.append(self._parse_car(car))

            if len(cars) == self.car_radius:
                break

        return tuple(cars)

    def _parse_observation(self, state):
        obs = {
            "pov_car": self._parse_car(state.pov_car),
            "other_cars": self._parse_cars(state.other_cars),
            "idle_passengers": self._parse_idle_passengers(state.idle_passengers),
        }

        assert self.observation_space.contains(obs)
        return obs

    def _parse_action(self, state: GridState, action: int):
        Action = self.rust.PyAction
        Direction = self.rust.Direction
        invalid_action = False

        if action < self.passengers_per_car:
            # pick up passenger with that index
            idx = action
            if idx < len(state.pov_car.passengers):
                return Action.drop_off_passenger(
                    state.pov_car.passengers[idx],
                )
            else:
                invalid_action = True

        elif action < self.action_count - 4:
            # drop off passenger
            idx = action - self.passengers_per_car
            if idx < len(state.idle_passengers):
                return Action.pick_up_passenger(state.idle_passengers[idx])
            else:
                invalid_action = True

        else:
            direction_idx = action - (self.action_count - 4)
            direction = {
                0: Direction.Up,
                2: Direction.Down,
                1: Direction.Right,
                3: Direction.Left,
            }[direction_idx]

            return Action.head_towards(direction)

        if invalid_action:
            return Action.head_towards(Direction.Up)


class CarCallback:
    def __init__(self, grid_env: GridVecEnv):
        self.grid_env = grid_env

        self.next_action = None
        self.transitions_arr: list[tuple | None] | None = None
        self.transitions_idx: int | None = None

    def get_action(self, state: GridState):
        assert self.next_action is not None

        action = self.grid_env._parse_action(state, self.next_action)
        self.next_action = None

        return action

    def transition_happened(self, state: GridState, action, new_state: GridState, reward: float):
        assert self.transitions_arr is not None
        assert self.transitions_idx is not None

        self.transitions_arr[self.transitions_idx] = (
            state,
            action,
            new_state,
            reward,
        )

        self.transitions_arr = None
        self.transitions_idx = None
