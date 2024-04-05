print("Importing main.py v2")

import time

import debugpy
from stable_baselines3 import A2C

from env import GridVecEnv, EnvOpts


def start(rust):
    start_debug()

    GridEnv = rust.PyGridEnv
    GridOpts = rust.GridOpts
    Action = rust.PyAction
    Direction = rust.Direction

    grid_opts = GridOpts(
        initial_passenger_count=10,
        passenger_spawn_rate=0.02,
        agent_car_count=2,
        npc_car_count=15,
    )
    env_opts = EnvOpts(
        passenger_radius=10,
        car_radius=10,
        passengers_per_car=4,
        render=False,
    )

    env = GridVecEnv(rust, grid_opts, env_opts)
    model = A2C("MlpPolicy", env, verbose=1, tensorboard_log="tensorboard_log")

    model.learn(total_timesteps=25000)

    # class CarAgent:
    #     def get_action(self, state):
    #         pov_car = state.pov_car

    #         if len(pov_car.passengers) > 0:
    #             passenger = pov_car.passengers[0]
    #             return Action.drop_off_passenger(passenger, None)

    #         if len(state.idle_passengers) > 0:
    #             closest_passenger = state.idle_passengers[0]
    #             return Action.pick_up_passenger(closest_passenger, None)

    #         return Action.head_towards(Direction.Up)

    #     def transition_happened(self, state, action, new_state, reward):
    #         print(f"transition_happened: {state=}, {action=}, {new_state=}, {reward=}")

    # agent = CarAgent()
    # env = GridEnv(agent, opts, render=True)

    # while True:
    #     env.tick()

    #     time.sleep(0.01)


def start_debug():
    debugpy.listen(("0.0.0.0", 5678), in_process_debug_adapter=True)
    print(f"Waiting for debugger on port 5678...")
    debugpy.debug_this_thread()
    debugpy.wait_for_client()
    print("Attached!")
    # debugpy.breakpoint()
