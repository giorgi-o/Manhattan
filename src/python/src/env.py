from pathlib import Path
import random
from dataclasses import dataclass
from itertools import chain
from typing import NewType, Any, Callable, TypeVar

import gymnasium
import numpy as np
import tianshou as ts

from gymnasium import spaces
from tianshou.env.venvs import BaseVectorEnv
from tianshou.env.worker import EnvWorker
from tianshou.env.utils import gym_new_venv_step_type
from tianshou.data import Batch


RustModule = NewType("RustModule", Any)  # type: ignore
GridEnv = NewType("GridEnv", Any)  # type: ignore
GridState = NewType("GridState", Any)  # type: ignore
GridOpts = NewType("GridOpts", Any)  # type: ignore
PyAction = NewType("PyAction", Any)  # type: ignore


@dataclass
class EnvOpts:
    render: bool


VARIANT = 3
AGENT_COUNT = 5
PASSENGER_COUNT = 30
SPAWN_MORE_PASSENGERS = True
TOPLEFT2BOTRIGHT = None
VERBOSE_AND_RENDER = True
DETERMINISTIC = False


# None = random
def generate_grid_opts(
    rust, variant: int | None = None, passengers: int | None = None, spawn_more_passengers: bool | None = None
):
    if variant is None:
        variant = VARIANT or random.randint(1, 3)
    if passengers is None:
        passengers = PASSENGER_COUNT or random.randint(10, 50)
    if spawn_more_passengers is None:
        spawn_more_passengers = SPAWN_MORE_PASSENGERS
        if spawn_more_passengers is None:
            spawn_more_passengers = random.random() > 0.7

    if TOPLEFT2BOTRIGHT is not None:
        topleft_2_botright = TOPLEFT2BOTRIGHT
    else:
        topleft_2_botright = random.random() > 0.7
    
    if spawn_more_passengers:
        passenger_spawn_rate = 0.005
    else:
        passenger_spawn_rate = 0.0

    charging_stations_pos = [
        rust.CarPosition(
            direction=rust.Direction.Right,
            road_index=4,
            section_index=5,
            position_in_section=2,
        ),
        rust.CarPosition(
            direction=rust.Direction.Right,
            road_index=5,
            section_index=7,
            position_in_section=2,
        ),
        rust.CarPosition(
            direction=rust.Direction.Down,
            road_index=8,
            section_index=6,
            position_in_section=2,
        ),
        rust.CarPosition(
            direction=rust.Direction.Right,
            road_index=2,
            section_index=9,
            position_in_section=2,
        ),
    ]

    if topleft_2_botright == True:
        passenger_events = [
            rust.PassengerEvent(
                start_area=(0, 0, 4, 4),
                destination_area=(-4, -4, -0.0, -0.0),
                spawn_rate=0.0,
                between_ticks=(None, None),
            )
        ]
    elif topleft_2_botright == False:
        passenger_events = []

    grid_opts_args = {}
    match variant:
        case 1:
            grid_opts_args["passengers_per_car"] = 1
            grid_opts_args["discharge_rate"] = 0.0
        case 2:
            grid_opts_args["passengers_per_car"] = 4
            grid_opts_args["discharge_rate"] = 0.0
        case 3:
            grid_opts_args["passengers_per_car"] = 4
            grid_opts_args["discharge_rate"] = 0.002

    grid_opts = rust.GridOpts(
        initial_passenger_count=passengers,
        passenger_spawn_rate=passenger_spawn_rate,
        max_passengers=30,
        agent_car_count=AGENT_COUNT,
        npc_car_count=200,
        # passengers_per_car=1,
        charging_stations=charging_stations_pos,
        charging_station_capacity=1,
        # discharge_rate=0.002,
        car_radius=3,
        passenger_radius=5,
        passenger_events=passenger_events,
        deterministic_mode=DETERMINISTIC,
        verbose=VERBOSE_AND_RENDER,
        **grid_opts_args,
    )

    return grid_opts


class GridVecEnv(BaseVectorEnv):
    def __init__(
        self,
        rust: RustModule,
        # grid_opts: GridOpts,
        # env_opts: EnvOpts,
    ):
        env_opts = EnvOpts(
            render=VERBOSE_AND_RENDER,
        )

        self.rust = rust
        self.grid_opts = generate_grid_opts(rust)
        self.env_opts = env_opts

        self.car_passenger_slots = 4
        self.num_envs = self.grid_opts.agent_car_count
        # self.width, self.height = rust.grid_dimensions()
        self.width, self.height = (15, 10)

        self.TICKS_PER_EPISODE = 10000
        self.MAX_DISTANCE = 100
        self.MAX_TIME = 300

        self.workers: list[GridEnvWorker] = []
        self.workers_ready_for_tick = np.array([False] * self.num_envs)
        self.workers_reset_called = np.array([False] * self.num_envs)

        # attributes required by tianshou to emulate us being a gym.Env
        self.is_closed = False

        # we hijack the API of BaseVectorEnv. It asks for a gymnasium env,
        # but we don't have one. Thing is, it doesn't actually use the env
        # directly, it only passes it onto the worker and lets the worker do
        # all the env interaction.
        # so since it will pass whatever we give it to the worker, we put a
        # (self, index) tuple instead of an actual env.
        # and as to why we create a "lambdaify" function, try running
        # >>> a = [(lambda: i) for i in range(10)]; a[0]()
        lambdaify = lambda *args: (lambda: args)
        env_fns = [lambdaify(self, i) for i in range(self.num_envs)]
        worker_fn = lambda env_fn: GridEnvWorker(env_fn)
        super().__init__(env_fns, worker_fn)  # type: ignore

    @property
    def passenger_radius(self) -> int:
        return self.grid_opts.passenger_radius

    @property
    def car_radius(self) -> int:
        return self.grid_opts.car_radius

    @property
    def passengers_per_car(self) -> int:
        return self.grid_opts.passengers_per_car
        # return 4

    @property
    def charging_station_count(self) -> int:
        # return len(self.grid_opts.charging_stations)
        return 4

    @property
    def charging_station_capacity(self) -> int:
        # return self.grid_opts.charging_station_capacity
        return 1

    def register_worker(self, worker: "GridEnvWorker"):
        assert len(self.workers) < self.num_envs
        self.workers.append(worker)

        if len(self.workers) == self.num_envs:
            # we got all the workers, now create the rust env
            self.create_rust_env()

    def create_rust_env(self):
        self.grid_opts = generate_grid_opts(self.rust)
        self.env = self.rust.PyGridEnv(self.workers, self.grid_opts, self.env_opts.render)

    def ready_for_tick(self, index: int):
        """A worker telling us it's ready for us to call rust.tick()
        Once all workers are ready, we'll call it
        """
        self.workers_ready_for_tick[index] = True

        if np.all(self.workers_ready_for_tick):
            self.env.tick()
            self.workers_ready_for_tick[:] = False

    def reset_called(self, index: int):
        self.workers_reset_called[index] = True

        if np.all(self.workers_reset_called):
            # before resetting, write stats
            self.write_episode_stats(self.workers[index])

            # for us, resetting is just creating a new rust env and
            # calling tick on it.
            # the workers know we just reset, so the first move will
            # be to head towards a random direction.
            # note: we need to tick once because the cars aren't
            # spawned on grid creation, only on first tick
            self.create_rust_env()
            self.env.tick()
            self.workers_reset_called[:] = False

    def write_episode_stats(self, worker: "GridEnvWorker"):
        if worker.old_obs is None:
            return

        py_state = worker.new_obs or worker.old_obs
        py_state.write_stats()

        # stats = py_state.stats

        # stats_file = Path(__file__).parent.parent.parent.parent / "logs" / "stats.csv"
        # if not stats_file.exists():
        #     csv_header = stats.csv_header()
        #     with open(stats_file, "w") as f:
        #         f.write(csv_header)

        # with open(stats_file, "a") as f:
        #     f.write(stats.csv_ify())

    @property
    def _observation_space(self) -> gymnasium.spaces.Box:
        # note: self.observation_space and self.action_space
        # (without _prefix) are taken by BaseVectorEnv

        if hasattr(self, "_observation_space_cache"):
            return self._observation_space_cache

        coords_ospc = [
            self.width,  # x
            self.height,  # y
            # 4,  # N/S/E/W direction
            self.MAX_DISTANCE + 1,  # distance to pov
        ]

        car_passenger_ospc = [
            2,  # present
            *coords_ospc,  # destination
            self.MAX_DISTANCE + 1,  # distance_to_dest
            self.MAX_TIME + 1,  # time_since_request
        ]
        car_passengers_ospc: list[int] = []
        for _ in range(self.car_passenger_slots):
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

        in_charging_station_ospc = [2]
        battery_ospc = [101]

        car_ospc = [
            *coords_ospc,  # pos
            *in_charging_station_ospc,
            *battery_ospc,  # battery
            *car_passengers_ospc,  # passengers
        ]
        cars_ospc: list[int] = []
        for _ in range(self.car_radius):
            cars_ospc.extend(car_ospc)

        charging_station_slot_ospc = [
            2,  # is available
            2,  # % charged of car that's there
        ]
        charging_station_slots_ospc: list[int] = charging_station_slot_ospc * self.charging_station_capacity

        charging_station_ospc = [
            *coords_ospc,  # pos
            *charging_station_slots_ospc,  # slots/other cars already there
        ]
        charging_stations_ospc: list[int] = charging_station_ospc * self.charging_station_count

        can_turn_spc = [2]  # whether the car's action this tick has an effect
        total_passengers_spc = [50]  # how many passengers are on the grid
        prev_action_spc = [2] * self.action_count  # which action did the car take last tick
        ticks_since_out_of_battery = [self.MAX_TIME]  # whether the car ran out of battery

        all_spaces = [
            *can_turn_spc,
            *total_passengers_spc,
            *car_ospc,
            *cars_ospc,
            *idle_passengers_ospc,
            # *prev_action_spc,
            *charging_stations_ospc,
            *ticks_since_out_of_battery,
        ]

        low = np.zeros(len(all_spaces), dtype=np.float32)
        high = np.array(all_spaces, dtype=np.float32) - 1.0

        self._observation_space_cache = spaces.Box(low=low, high=high, dtype=np.float32)
        return self._observation_space_cache

    @property
    def action_count(self) -> int:
        return (
            self.car_passenger_slots  # drop off passenger
            + self.passenger_radius  # pick up passenger
            + 1  # go to nearest charging station
            + 4  # head towards N/S/E/W
        )

    @property
    def _action_space(self) -> gymnasium.spaces.Discrete:
        return spaces.Discrete(self.action_count)

    def parse_action(self, state: GridState, action: int) -> PyAction:
        Action = self.rust.PyAction
        Direction = self.rust.Direction
        low = 0

        if action < self.car_passenger_slots:
            # drop off passenger with that index
            idx = action
            assert idx < len(state.pov_car.passengers)
            return Action.drop_off_passenger(state.pov_car.passengers[idx], action, idx)
        low += self.car_passenger_slots

        if action < low + self.passenger_radius:
            # pick up passenger
            idx = action - low
            assert idx < len(state.idle_passengers)

            can_pick_up_passengers = len(state.pov_car.passengers) < self.passengers_per_car
            assert can_pick_up_passengers

            idle_passenger = state.idle_passengers[idx]
            return Action.pick_up_passenger(idle_passenger, action, idx)
        low += self.passenger_radius

        if action < low + 1:
            # go to charging station
            idx = action - low
            assert idx < len(state.charging_stations)

            charging_station = state.charging_stations[idx]
            return Action.charge_battery(charging_station, action)
        low += 1

        if action < low + 4:
            # head towards direction
            direction_idx = action - low
            direction = {
                0: Direction.Up,
                2: Direction.Down,
                1: Direction.Right,
                3: Direction.Left,
            }[direction_idx]

            return Action.head_towards(direction, action)

        raise ValueError(f"Invalid action {action}")
        # if parsed_action is not None:
        #     return parsed_action, True
        # else:
        #     return Action.head_towards(Direction.Up, action), False

    def calculate_reward(
        self, old_state: GridState, new_state: GridState, action_valid: bool = True
    ) -> float:
        pov_car = new_state.pov_car
        reward = 0.0

        total_passengers = (old_state or new_state).total_passenger_count()
        if total_passengers == 0:
            # we win: +5000
            reward += 5000
        else:
            # -1 for every passenger on the grid
            # reward -= total_passengers
            reward -= 1

        # for passenger in chain(state.idle_passengers, state.pov_car.passengers):
        #     # penalty of "time alive" / 100
        #     reward -= passenger.ticks_since_request / 1000

        events = new_state.events
        events_reward = 0.0

        # +100 for every passenger dropped off
        events_reward += 100 * len(events.car_dropped_off_passenger)

        # +5 for every passenger picked up
        events_reward += 5 * len(events.car_picked_up_passenger)

        # -500 if the car ran out of battery
        events_reward -= 500 * len(events.car_out_of_battery)

        reward += events_reward

        if len(pov_car.recent_actions) > 0:
            action = pov_car.recent_actions[0]

            # -1 if action is head_towards
            if action.is_head_towards():
                reward -= 1

            #     else:
            #         # +3 if action is drop_off
            #         if action.is_drop_off():
            #             reward += 3

            #         # +3 for every consecutive time the agent picked this action
            #         for prev_action in pov_car.recent_actions[1:2]:
            #             if prev_action == action:
            #                 reward += 3
            #             else:
            #                 break

            # if pov_car.battery < 0.1:
            #     if action.is_charge():
            #         reward += 0.1
            #     else:
            #         reward -= 20

            # if action.is_charge():
            # reward += 0.1

        #     # +3 if car wants to charge when battery is <30%
        #     if state.pov_car.battery < 0.13:
        #         reward += 3

        #     # +10 if car wants to keep charging if already doing so
        #     if state.pov_car.pos.in_charging_station:
        #         reward += 10

        # else:
        #     # -20 if battery is <30% and the car doesn't want to charge
        #     if state.pov_car.battery < 0.30:
        #         reward -= 20

        # if pov_car.pos.in_charging_station:
        #     # # +0.1 if car is charging
        #     # reward += 0.1
        #     if pov_car.battery < 0.1:
        #         reward += 0.1

        if self.grid_opts.verbose:
            print(f"{reward:.1f}", end=" ", flush=True)
        return reward

    # def _parse_direction(self, direction) -> int:
    #     Direction = self.rust.Direction
    #     match direction:
    #         case Direction.Up:
    #             return 0
    #         case Direction.Right:
    #             return 1
    #         case Direction.Down:
    #             return 2
    #         case Direction.Left:
    #             return 3
    #         case _:
    #             raise ValueError(f"Invalid direction: {direction}")

    def _parse_coords(self, coords, pov_pos) -> list[int | float]:
        Direction = self.rust.Direction

        horizontal = coords.direction in [Direction.Right, Direction.Left]
        if horizontal:
            x, y = (coords.section, coords.road)
        else:
            x, y = (coords.road, coords.section)
        assert x < self.width and y < self.height

        # direction = self._parse_direction(coords.direction)
        distance_to_pov = self.rust.calculate_distance(coords, pov_pos)
        distance_to_pov = min(distance_to_pov, self.MAX_DISTANCE)
        return [x, y, distance_to_pov]

    def _null_coords(self) -> list[int]:
        return [0, 0, 0]

    def _null_passenger(self) -> list[int]:
        return [
            0,  # can pick up
            *self._null_coords(),  # destination
            0,  # distance_to_dest
            0,  # time_since_request
        ]

    def _parse_car_passengers(self, car) -> list[int | float]:
        passengers = []

        for passenger in car.passengers:
            can_pick_up = len(car.passengers) < self.passengers_per_car
            parsed_passenger = [
                can_pick_up,
                *self._parse_coords(passenger.destination, car.pos),  # destination
                min(passenger.distance_to_destination, self.MAX_DISTANCE),  # distance_to_dest
                min(passenger.ticks_since_request, self.MAX_TIME),  # time_since_request
            ]
            passengers.extend(parsed_passenger)

            if len(passengers) == self.passengers_per_car:
                break

        null_passenger = self._null_passenger()
        neurons_per_passenger = len(null_passenger)
        while len(passengers) < self.car_passenger_slots * neurons_per_passenger:
            passengers.extend(null_passenger)

        return passengers

    def _parse_idle_passengers(self, idle_passengers, pov_car) -> list[int | float]:
        parsed_idle_passengers = []

        for passenger in idle_passengers:
            parsed_idle_passenger = [
                1,  # present
                *self._parse_coords(passenger.pos, pov_car.pos),  # pos
                *self._parse_coords(passenger.destination, pov_car.pos),  # destination
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

    def _parse_car(self, car) -> list[int | float]:
        return [
            *self._parse_coords(car.pos, car.pos),  # pos
            1 if car.pos.in_charging_station else 0,
            max(car.battery * 100, 0.0),
            *self._parse_car_passengers(car),
        ]

    def _null_car(self) -> list[int]:
        passengers = [self._null_passenger()] * self.car_passenger_slots
        return [
            *self._null_coords(),  # pos
            0,  # in charging station
            0,  # battery
            *flatten(passengers),
        ]

    def _parse_cars(self, cars) -> list[int | float]:
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

    def _parse_charging_station(self, charging_station, pov_car) -> list[int | float]:
        slots = [0, 0] * self.charging_station_capacity
        # for i, car in enumerate(charging_station.cars):
        if len(charging_station.cars) == 4:
            i = 0
            car = charging_station.cars[0]
            i = i * 2
            slots[i] = 1
            slots[i + 1] = max(car.battery, 0)

        return [
            *self._parse_coords(charging_station.pos, pov_car.pos),  # pos
            *slots,
        ]

    def _parse_charging_stations(self, state) -> list[int | float]:
        # return [*self._parse_charging_station(cs, state.pov_car) for cs in state.charging_stations]
        charging_stations = []
        for i, cs in enumerate(state.charging_stations):
            parsed_cs = self._parse_charging_station(cs, state.pov_car)
            charging_stations.extend(parsed_cs)

            if i + 1 == self.charging_station_count:
                break

        return charging_stations

    def parse_observation(self, state: GridState) -> np.ndarray:

        can_turn = 1 if state.can_turn else 0
        total_passenger_count = state.total_passenger_count()
        total_passenger_count = min(total_passenger_count, 49)
        ticks_since_out_of_battery = min(state.pov_car.ticks_since_out_of_battery, self.MAX_TIME - 1)

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
            *self._parse_idle_passengers(state.idle_passengers, state.pov_car),  # idle_passengers
            # *previous_actions,
            *self._parse_charging_stations(state),
            ticks_since_out_of_battery,
        ]

        obs = np.array(obs_list, dtype=np.float32)

        assert self._observation_space.contains(obs)
        return obs

    def action_mask(self, state: GridState) -> np.ndarray:
        """Returns an array the size of the action space, with True if
        that action is valid and False if it isn't."""
        valid_actions = np.array([False] * self.action_count)

        active_action = state.pov_car.active_action
        if active_action is not None:
            # the agent's last action has not been fulfilled yet,
            # keep picking it

            # find which neuron it corresponds to
            # idx = next(i for i in range(self.action_count) if self.parse_action(state, i) == active_action)
            idx = None
            for i in range(self.action_count):
                try:
                    if self.parse_action(state, i) == active_action:
                        idx = i
                        break
                except AssertionError:  # invalid action index
                    pass

            # idx might be None e.g. if the passenger just got picked
            # up by another car
            if idx is not None:
                valid_actions[idx] = True
                return valid_actions

        offset = 0

        # drop off actions
        passenger_count = len(state.pov_car.passengers)
        valid_actions[offset : offset + passenger_count] = True
        offset += self.car_passenger_slots

        # pick up actions
        can_pick_up_passengers = passenger_count < self.passengers_per_car
        if can_pick_up_passengers:
            idle_passenger_count = min(len(state.idle_passengers), self.passenger_radius)
            valid_actions[offset : offset + idle_passenger_count] = True
        offset += self.passenger_radius

        # charging station actions
        charging_station_offset = offset
        current_battery_level = state.pov_car.battery
        is_in_charging_station = state.pov_car.pos.in_charging_station

        # tmp: disable charging for now
        # if current_battery_level > 0.95:
        #     pass  # battery is full, don't start charging
        # # elif current_battery_level < 0.1:
        # #     # hack: only allow charging
        # #     valid_actions[:] = False
        # #     valid_actions[offset] = True
        # elif is_in_charging_station:
        #     # allow keep charging, but don't allow going to the other one
        #     valid_actions[offset] = True
        # else:
        #     # tmp: can always go to charging station, will just
        #     # hover in front if full
        #     # valid_actions[offset] = True
        #     for i, charging_station in enumerate(state.charging_stations):
        #         if not charging_station.is_full():
        #             valid_actions[offset + i] = True
        #     # break  # tmp-ish: only allow charging at nearest charging station
        # # if valid_actions[offset] == False:
        #     # print("Car can't go to cs!", flush=True)
        offset += 1  # self.charging_station_count

        # head towards actions (always valid)
        # valid_actions[offset : offset + 4] = True
        # valid only if nothing else is valid
        if not valid_actions.any():
            valid_actions[offset : offset + 4] = True
        offset += 4

        if is_in_charging_station:
            if current_battery_level < 0.98:
                # force charge till completion
                valid_actions[:] = False
                valid_actions[charging_station_offset] = True
        else:
            if current_battery_level < 0.98:
                # can go to charging station if it wants
                valid_actions[charging_station_offset] = True

        if current_battery_level < 0.98:
            valid_actions[charging_station_offset] = True

        assert offset == self.action_count
        return valid_actions


class GridEnvWorker(EnvWorker):
    """Class for managing a single car in the env.
    Order in which the functions will be called:
    - ts will call send() with the action for this car
    - we will call vec_env.ready_for_tick()
    - once all workers are ready, vec_env will call rust.tick()
    - rust will call get_action() to get the action
    - rust will call transition_happened() to send us the new state
    - we will calculate the reward
    - ts will call recv() to get the whole transition
    """

    def __init__(self, env_fn: Callable[[], tuple[GridVecEnv, int]]) -> None:
        self.vec_env, self.index = env_fn()
        self.vec_env.register_worker(self)
        self.rust = self.vec_env.rust

        self.action: PyAction | None = None
        self.action_valid: bool = True  # note: unused
        self.old_obs: GridState | None = None
        self.new_obs: GridState | None = None
        self.reward = 0.0
        self.reset_called = False

        self.pending_events_score = 0.0

        super().__init__(lambda: None)  # type: ignore

    def random_direction_action(self) -> PyAction:
        Dir = self.rust.Direction
        direction = random.choice([Dir.Up, Dir.Down, Dir.Left, Dir.Right])
        return self.rust.PyAction.head_towards(direction, None)

    # === TIANSHOU FUNCTIONS ===

    def send(self, action: np.ndarray | None) -> None:
        if action is None:  # reset commmand
            self.action = self.random_direction_action()
            self.reset_called = True
            self.vec_env.reset_called(self.index)

        else:
            self.reset_called = False
            if self.vec_env.grid_opts.verbose:
                print(int(action), end=" ", flush=True)

            assert self.new_obs is not None  # since first tick already happened
            self.action = self.vec_env.parse_action(self.new_obs, int(action))
            self.vec_env.ready_for_tick(self.index)

    def recv(self) -> gym_new_venv_step_type | tuple[np.ndarray, dict]:
        # if reset was called, only return (obs, info)
        # otherwise, return (obs, rew, terminated, truncated, info)
        assert self.new_obs is not None
        assert self.reward is not None

        info = {}
        parsed_new_obs = self.vec_env.parse_observation(self.new_obs)

        # tmp: print car battery
        # todo

        new_obs_action_mask = self.vec_env.action_mask(self.new_obs)
        new_obs_batch = Batch(
            obs=parsed_new_obs,
            mask=new_obs_action_mask,
            # need to add a non-numpy attribute, otherwise when tianshou
            # tries to np.stack the observation batches together, it gets
            # confused and goes along the wrong axis (or something)
            random_attr=EmptyClass(),
        )

        if self.reset_called:
            return new_obs_batch, info  # type: ignore
        else:
            terminated = self.new_obs.total_passenger_count() == 0
            truncated = self.new_obs.ticks_passed >= self.vec_env.TICKS_PER_EPISODE

            return (  # type: ignore
                new_obs_batch,
                np.array(self.reward),
                np.array(terminated),
                np.array(truncated),
                info,
            )

    # === RUST FUNCTIONS ===
    def get_action(self, state: GridState) -> PyAction:
        self.old_obs = state

        assert self.action is not None
        action = self.action
        self.action = None

        return action

    def transition_happened(self, old_state: GridState, new_state: GridState):
        # note: old_state won't be the same as self.old_obs if reset is called
        # I think because old_state will be the new state without the cars, but
        # self.old_obs will be the one from the previous env

        self.new_obs = new_state
        self.reward = self.vec_env.calculate_reward(old_state, new_state, self.action_valid)
        self.reward += self.pending_events_score

    # === USELESS ABC FUNCTIONS ===

    def get_env_attr(self, key: str) -> Any:
        if key == "action_space":
            return self.vec_env._action_space
        elif key == "observation_space":
            return self.vec_env._observation_space

        raise NotImplementedError

    def set_env_attr(self, key: str, value: Any) -> None:
        raise NotImplementedError

    def reset(self, **kwargs: Any) -> tuple[np.ndarray, dict]:
        raise NotImplementedError

    def render(self, **kwargs: Any) -> Any:
        """Render the environment."""
        raise NotImplementedError

    def close_env(self) -> None:
        raise NotImplementedError


T = TypeVar("T")


def flatten(x: list[list[T]]) -> list[T]:
    return [item for sublist in x for item in sublist]


class EmptyClass:
    pass
