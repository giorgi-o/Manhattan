print("Importing main.py")


import time

from dqn.dqn import DQN
from environment.manhattan import ManhattanEnv


def hello_world():
    print("Hello World")


# def agent(input: list[int]):
#     print(f"agent(): {input}")

#     # return index of lowest number
#     return input.index(min(input))


def start(rust):

    print(f"{rust.Grid=}")

    env = ManhattanEnv(rust, agent_cars=1, npc_cars=50, passenger_radius=10, render=True)
    dqn = DQN(environment=env, episode_count=1000, timestep_count=100000, gamma=1.0)

    rust.set_agent_callback(env.agent_callback)
    rust.set_transition_callback(env.timestep_callback)

    dqn.launch()

    # grid = rust.Grid(render=True)
    # print(f"{grid=}")

    # grid.add_agent_car(callback)
    # for _ in range(200):
    #     grid.add_npc_car()

    # TPS = 20
    # last_tick = time.time()

    # while not grid.done():
    #     grid.tick()

    # now = time.time()
    # if now - last_tick < 1 / TPS:
    #     time.sleep(1 / TPS - (now - last_tick))
    #     pass
    # last_tick = now


# def callback(*kwargs):
#     print(f"callback: {kwargs}")
