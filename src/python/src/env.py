import random
from dataclasses import dataclass
from typing import NewType, Any, TypeVar

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
        self.passengers_per_car = grid_opts.passengers_per_car

        self.width, self.height = rust.grid_dimensions()

        self.render_mode = None  # we handle rendering ourselves
        self.TICKS_PER_EPISODE = 2000
        self.MAX_DISTANCE = 100
        self.MAX_TIME = 300

        num_envs = grid_opts.agent_car_count
        action_space = self._action_space()
        observation_space = self._observation_space()
        super().__init__(num_envs, observation_space, action_space)

        self.callbacks = [CarCallback(self) for _ in range(num_envs)]
        # self.reset()
        self.env: GridEnv

    def step(self, actions: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray, list[dict]]:
        transitions: list[tuple | None] = [None] * self.num_envs

        for i in range(self.num_envs):
            cb = self.callbacks[i]

            cb.next_action = actions[i]
            cb.transitions_arr = transitions
            cb.transitions_idx = i

        self.env.tick()

        obs_shape = self.observation_space.shape
        assert obs_shape is not None

        observations = np.zeros((self.num_envs, *obs_shape))
        rewards = np.zeros(self.num_envs, dtype=np.float32)
        dones = np.zeros(self.num_envs, dtype=bool)
        infos = [{}] * self.num_envs

        print("Rewards:", end=" ")

        for i, transition in enumerate(transitions):
            assert transition is not None
            old_state, action_valid, new_state = transition
            reward = self._calculate_reward(new_state, action_valid)

            observation = self._parse_observation(new_state)
            observations[i] = observation
            rewards[i] = reward

            truncate = new_state.ticks_passed >= self.TICKS_PER_EPISODE
            if truncate:
                dones[i] = True
                infos[i]["terminal_observation"] = observation
                infos[i]["TimeLimit.truncated"] = True

        print()

        # need to auto-reset the env once truncated
        if np.any(dones):
            observations = self.reset()

        return np.array(observations), rewards, dones, infos

    def reset(self) -> np.ndarray:
        self.env = self.rust.PyGridEnv(self.callbacks, self.grid_opts, self.env_opts.render)

        Dir = self.rust.Direction
        random_direction = lambda: random.choice([Dir.Up, Dir.Down, Dir.Left, Dir.Right])

        head_towards = lambda d: self.rust.PyAction.head_towards(d, None)
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

    def _calculate_reward(self, state: GridState, action_valid: bool) -> float:
        pov_car = state.pov_car
        reward = 0.0

        # -1 for every passenger on the grid
        reward -= state.total_passenger_count()

        # +100 for every passenger dropped off
        reward += 100 * len(state.events.car_dropped_off_passenger)

        # +10 for every passenger picked up
        reward += 10 * len(state.events.car_picked_up_passenger)

        # -50 if the action is invalid
        if not action_valid:
            reward -= 50
        elif len(pov_car.recent_actions) > 0:
            action = pov_car.recent_actions[0]

            # -10 if action is head_towards
            if action.is_head_towards():
                reward -= 10

            else:
                # +3 if action is drop_off
                if action.is_drop_off():
                    reward += 3

                # +3 for every consecutive time the agent picked this action
                for prev_action in pov_car.recent_actions[1:2]:
                    if prev_action == action:
                        reward += 1
                    else:
                        break

        print(f"{reward}", end=" ")
        return reward

    def _observation_space(self) -> gym.Space:
        coords_ospc = [
            self.width,  # x
            self.height,  # y
            4,  # N/S/E/W direction
        ]

        car_passenger_ospc = [
            2,  # present
            *coords_ospc,  # destination
            self.MAX_DISTANCE + 1,  # distance_to_dest
            self.MAX_TIME + 1,  # time_since_request
        ]
        car_passengers_ospc: list[int] = []
        for _ in range(self.passengers_per_car):
            car_passengers_ospc.extend(car_passenger_ospc)

        idle_passenger_ospc = [
            2,  # present
            *coords_ospc,  # pos
            *coords_ospc,  # destination
            self.MAX_DISTANCE + 1,  # distance_to_dest
            self.MAX_TIME + 1,  # time_since_request
        ]
        idle_passengers_ospc: list[int] = []
        for _ in range(self.passenger_radius):
            idle_passengers_ospc.extend(idle_passenger_ospc)

        car_ospc = [
            *coords_ospc,  # pos
            *car_passengers_ospc,  # passengers
        ]
        cars_ospc: list[int] = []
        for _ in range(self.car_radius):
            cars_ospc.extend(car_ospc)

        can_turn_spc = [2]  # whether the car's action this tick has an effect
        total_passengers_spc = [50]  # how many passengers are on the grid
        prev_action_spc = [2] * self.action_count  # which action did the car take last tick

        all_spaces = [
            *can_turn_spc,
            *total_passengers_spc,
            *car_ospc,
            *cars_ospc,
            *idle_passengers_ospc,
            *prev_action_spc,
        ]
        return spaces.MultiDiscrete(all_spaces)

    def _action_space(self) -> gym.spaces.Discrete:
        # can pick passenger to drop off, pick up, or to head N/S/E/W
        self.action_count = self.passengers_per_car + self.passenger_radius + 4
        return spaces.Discrete(self.action_count)

    def _parse_direction(self, direction) -> int:
        Direction = self.rust.Direction
        match direction:
            case Direction.Up:
                return 0
            case Direction.Right:
                return 1
            case Direction.Down:
                return 2
            case Direction.Left:
                return 3
            case _:
                raise ValueError(f"Invalid direction: {direction}")

    def _parse_coords(self, coords) -> list[int]:
        Direction = self.rust.Direction

        horizontal = coords.direction in [Direction.Right, Direction.Left]
        if horizontal:
            x, y = (coords.section, coords.road)
        else:
            x, y = (coords.road, coords.section)
        assert x < self.width and y < self.height

        direction = self._parse_direction(coords.direction)
        return [x, y, direction]

    def _null_coords(self) -> list[int]:
        return [0, 0, 0]

    def _null_passenger(self) -> list[int]:
        return [
            0,  # can pick up
            *self._null_coords(),  # destination
            0,  # distance_to_dest
            0,  # time_since_request
        ]

    def _parse_car_passengers(self, car) -> list[int]:
        passengers = []

        for passenger in car.passengers:
            can_pick_up = len(car.passengers) < self.passengers_per_car
            parsed_passenger = [
                can_pick_up,
                *self._parse_coords(passenger.destination),  # destination
                min(passenger.distance_to_destination, self.MAX_DISTANCE),  # distance_to_dest
                min(passenger.ticks_since_request, self.MAX_TIME),  # time_since_request
            ]
            passengers.append(parsed_passenger)

            if len(passengers) == self.passengers_per_car:
                break

        null_passenger = self._null_passenger()
        while len(passengers) < self.passengers_per_car:
            passengers.append(null_passenger)

        # return flattened passengers
        return [item for sublist in passengers for item in sublist]

    def _parse_idle_passengers(self, idle_passengers) -> list[int]:
        parsed_idle_passengers = []

        for passenger in idle_passengers:
            parsed_idle_passenger = [
                1,  # present
                *self._parse_coords(passenger.pos),  # pos
                *self._parse_coords(passenger.destination),  # destination
                min(passenger.distance_to_destination, self.MAX_DISTANCE),  # distance_to_dest
                min(passenger.ticks_since_request, self.MAX_TIME),  # time_since_request
            ]
            parsed_idle_passengers.append(parsed_idle_passenger)

            if len(parsed_idle_passengers) == self.passenger_radius:
                break

        null_idle_passenger = [
            0,  # present
            *self._null_coords(),  # pos
            *self._null_coords(),  # destination
            0,  # distance_to_dest
            0,  # time_since_request
        ]
        while len(parsed_idle_passengers) < self.passenger_radius:
            parsed_idle_passengers.append(null_idle_passenger)

        return flatten(parsed_idle_passengers)

    def _parse_car(self, car) -> list[int]:
        return [
            *self._parse_coords(car.pos),  # pos
            *self._parse_car_passengers(car),
        ]

    def _null_car(self) -> list[int]:
        passengers = [self._null_passenger()] * self.passengers_per_car
        return [
            *self._null_coords(),  # pos
            *flatten(passengers),
        ]

    def _parse_cars(self, cars) -> list[int]:
        cars = []

        for car in cars:
            if car.ty != self.rust.PyCarType.Agent:
                continue

            cars.append(self._parse_car(car))

            if len(cars) == self.car_radius:
                break

        null_car = self._null_car()
        while len(cars) < self.car_radius:
            cars.append(null_car)

        return flatten(cars)

    def _parse_observation(self, state) -> np.ndarray:
        can_turn = 1 if state.can_turn else 0
        total_passenger_count = state.total_passenger_count()
        total_passenger_count = min(total_passenger_count, 49)

        previous_actions = np.zeros(self.action_count)
        if len(state.pov_car.recent_actions) > 0:
            prev_action_idx = state.pov_car.recent_actions[0].raw
            if prev_action_idx is not None:
                previous_actions[prev_action_idx] = 1

        obs_list = [
            can_turn,
            total_passenger_count,
            *self._parse_car(state.pov_car),  # pov_car
            *self._parse_cars(state.other_cars),  # other_cars
            *self._parse_idle_passengers(state.idle_passengers),  # idle_passengers
            *previous_actions,
        ]

        obs = np.array(obs_list)

        assert self.observation_space.contains(obs)
        return obs

    def _parse_action(self, state: GridState, action: int) -> tuple[Any, bool]:
        Action = self.rust.PyAction
        Direction = self.rust.Direction
        parsed_action = None

        if action < self.passengers_per_car:
            # drop off passenger with that index
            idx = action
            if idx < len(state.pov_car.passengers):
                parsed_action = Action.drop_off_passenger(state.pov_car.passengers[idx], action)

        elif action < self.action_count - 4:
            # pick up passenger
            idx = action - self.passengers_per_car
            can_pick_up_passengers = len(state.pov_car.passengers) < self.passengers_per_car
            if idx < len(state.idle_passengers) and can_pick_up_passengers:
                parsed_action = Action.pick_up_passenger(state.idle_passengers[idx], action)

        else:
            direction_idx = action - (self.action_count - 4)
            direction = {
                0: Direction.Up,
                2: Direction.Down,
                1: Direction.Right,
                3: Direction.Left,
            }[direction_idx]

            parsed_action = Action.head_towards(direction, action)

        if parsed_action is not None:
            return parsed_action, True
        else:
            return Action.head_towards(Direction.Up, action), False


class CarCallback:
    def __init__(self, grid_env: GridVecEnv):
        self.grid_env = grid_env

        self.next_action = None
        self.action_valid = True

        self.transitions_arr: list[tuple | None] | None = None
        self.transitions_idx: int | None = None

    def get_action(self, state: GridState):
        assert self.next_action is not None

        action, action_valid = self.grid_env._parse_action(state, self.next_action)
        self.action_valid = action_valid
        self.next_action = None

        return action

    def transition_happened(self, state: GridState, new_state: GridState):
        assert self.transitions_arr is not None
        assert self.transitions_idx is not None

        self.transitions_arr[self.transitions_idx] = (
            state,
            self.action_valid,
            new_state,
        )

        self.transitions_arr = None
        self.transitions_idx = None


T = TypeVar("T")


def flatten(x: list[list[T]]) -> list[T]:
    return [item for sublist in x for item in sublist]


def debug():
    import debugpy

    debugpy.listen(("0.0.0.0", 5678), in_process_debug_adapter=True)
    print(f"Waiting for debugger on port 5678...")
    debugpy.debug_this_thread()
    debugpy.wait_for_client()
    print("Attached!")
    debugpy.breakpoint()
