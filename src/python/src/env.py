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

    def step(self, actions: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray, list[dict]]:
        transitions: list[tuple | None] = [None] * self.num_envs

        for i in range(self.num_envs):
            cb = self.callbacks[i]

            cb.next_action = actions[i]
            cb.transitions_arr = transitions
            cb.transitions_idx = i

        self.env.tick()

        observations: list[np.ndarray] = []
        rewards = []
        done = np.array([False] * self.num_envs)
        information = [{}] * self.num_envs

        for transition in transitions:
            assert transition is not None
            old_state, action, new_state, reward = transition

            observation = self._parse_observation(new_state)
            observations.append(observation)
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
        # coords_ospc = spaces.Dict(
        #     {
        #         "x": spaces.Discrete(self.width),
        #         "y": spaces.Discrete(self.height),
        #         "direction": direction_spc,
        #     }
        # )
        coords_ospc = (
            spaces.Discrete(self.width),  # x
            spaces.Discrete(self.height),  # y
            spaces.Discrete(4)  # N/S/E/W direction
        )

        # car_passenger_ospc = spaces.Dict(
        #     {
        #         "present": spaces.Discrete(2),
        #         "destination": coords_ospc,
        #         "distance_to_dest": spaces.Discrete(100),
        #         "time_since_request": spaces.Discrete(300),
        #     }
        # )
        car_passenger_ospc = (
            spaces.Discrete(2),  # present
            *coords_ospc,  # destination
            spaces.Discrete(100),  # distance_to_dest
            spaces.Discrete(300),  # time_since_request
        )
        # car_passengers_ospc = spaces.Tuple((car_passenger_ospc,) * self.passengers_per_car)
        # car_passengers_ospc = [car_passenger_ospc for _ in range(self.passengers_per_car)]
        car_passengers_ospc: list[spaces.Discrete] = []
        for _ in range(self.passengers_per_car):
            car_passengers_ospc.extend(car_passenger_ospc)

        # idle_passenger_ospc = spaces.Dict(
        #     {
        #         "present": spaces.Discrete(2),
        #         "pos": coords_ospc,
        #         "destination": coords_ospc,
        #         "distance_to_dest": spaces.Discrete(100),
        #         "time_since_request": spaces.Discrete(300),
        #     }
        # )
        idle_passenger_ospc = (
            spaces.Discrete(2),  # present
            *coords_ospc,  # pos
            *coords_ospc,  # destination
            spaces.Discrete(100),  # distance_to_dest
            spaces.Discrete(300),  # time_since_request
        )
        # idle_passengers_ospc = spaces.Tuple((idle_passenger_ospc,) * self.passenger_radius)
        # idle_passengers_ospc = [idle_passenger_ospc for _ in range(self.passenger_radius)]
        idle_passengers_ospc: list[spaces.Discrete] = []
        for _ in range(self.passenger_radius):
            idle_passengers_ospc.extend(idle_passenger_ospc)

        # car_ospc = spaces.Dict(
        #     {
        #         "pos": coords_ospc,  # pos
        #         "passengers": car_passengers_ospc,
        #     }
        # )
        car_ospc = (
            *coords_ospc,  # pos
            *car_passengers_ospc,  # passengers
        )
        # cars_ospc = spaces.Tuple((car_ospc,) * (self.car_radius))
        # cars_ospc = [car_ospc for _ in range(self.car_radius)]
        cars_ospc: list[spaces.Discrete] = []
        for _ in range(self.car_radius):
            cars_ospc.extend(car_ospc)

        # return spaces.Dict(
        #     {
        #         "pov_car": car_ospc,
        #         "other_cars": cars_ospc,
        #         "idle_passengers": idle_passengers_ospc,
        #     }
        # )

        # return spaces.Tuple([*car_ospc, *cars_ospc, *idle_passengers_ospc])
        all_spaces = [*car_ospc, *cars_ospc, *idle_passengers_ospc]
        # return spaces.Dict({str(i): all_spaces[i] for i in range(len(all_spaces))})
        return spaces.Tuple(all_spaces)

    def _action_space(self) -> gym.Space:
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
            0,  # present
            *self._null_coords(),  # destination
            0,  # distance_to_dest
            0,  # time_since_request
        ]

    def _parse_car_passengers(self, car) -> list[int]:
        passengers = []

        for passenger in car.passengers:
            # passengers.append(
            #     {
            #         "present": 1,
            #         "destination": self._parse_coords(passenger.destination),
            #         "distance_to_dest": passenger.distance_to_destination,
            #         "time_since_request": passenger.ticks_since_request,
            #     }
            # )
            parsed_passenger = [
                1,  # present
                *self._parse_coords(passenger.destination),  # destination
                passenger.distance_to_destination,  # distance_to_dest
                passenger.ticks_since_request,  # time_since_request
            ]
            passengers.append(parsed_passenger)

        null_passenger = self._null_passenger()
        while len(passengers) < self.passengers_per_car:
            passengers.append(null_passenger)

        # return flattened passengers
        return [item for sublist in passengers for item in sublist]

    def _parse_idle_passengers(self, idle_passengers) -> list[int]:
        parsed_idle_passengers = []

        for passenger in idle_passengers:
            # parsed_idle_passengers.append(
            #     {
            #         "present": 1,
            #         "pos": self._parse_coords(passenger.pos),
            #         "destination": self._parse_coords(passenger.destination),
            #         "distance_to_dest": passenger.distance_to_destination,
            #         "time_since_request": passenger.ticks_since_request,
            #     }
            # )
            parsed_idle_passenger = [
                1,  # present
                *self._parse_coords(passenger.pos),  # pos
                *self._parse_coords(passenger.destination),  # destination
                passenger.distance_to_destination,  # distance_to_dest
                passenger.ticks_since_request,  # time_since_request
            ]
            parsed_idle_passengers.append(parsed_idle_passenger)

            if len(idle_passengers) == self.passenger_radius:
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
        # return {
        #     "pos": self._parse_coords(car.position),
        #     "passengers": self._parse_car_passengers(car),
        # }
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

    def _parse_observation(self, state) -> dict[str, int]:
        # obs = {
        #     "pov_car": self._parse_car(state.pov_car),
        #     "other_cars": self._parse_cars(state.other_cars),
        #     "idle_passengers": self._parse_idle_passengers(state.idle_passengers),
        # }
        obs_list = [
            *self._parse_car(state.pov_car),  # pov_car
            *self._parse_cars(state.other_cars),  # other_cars
            *self._parse_idle_passengers(state.idle_passengers),  # idle_passengers
        ]

        # obs = {str(x): y for x, y in enumerate(obs_list)}
        obs = tuple(obs_list)

        # debug()
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
