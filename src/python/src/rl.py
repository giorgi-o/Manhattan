import argparse
import os
import pprint
from pathlib import Path

import gymnasium as gym
import numpy as np
import torch
from torch.utils.tensorboard.writer import SummaryWriter

from tianshou.data import (
    Collector,
    PrioritizedVectorReplayBuffer,
    ReplayBuffer,
    VectorReplayBuffer,
)
from tianshou.env import DummyVectorEnv
from tianshou.policy import RainbowPolicy, FQFPolicy
from tianshou.policy.base import BasePolicy
from tianshou.trainer import OffpolicyTrainer
from tianshou.utils import TensorboardLogger
from tianshou.utils.net.common import Net
from tianshou.utils.net.discrete import FullQuantileFunction, FractionProposalNetwork
from tianshou.utils.space_info import SpaceInfo

from env import GridVecEnv
from util import LogStep


def dqn(env: GridVecEnv) -> None:
    space_info = SpaceInfo.from_spaces(env._action_space, env._observation_space)

    # seed
    seed = 42
    np.random.seed(seed)
    torch.manual_seed(seed)
    env.seed(seed)

    # model
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")

    with LogStep("Creating feature network..."):
        feature_net = Net(
            state_shape=space_info.observation_info.obs_shape,
            action_shape=64,
            hidden_sizes=[64, 64],
            device=device,
            softmax=False,
            # softmax=True,
            # num_atoms=51,
        ).to(device)

    with LogStep("Creating full quantile function..."):
        net = FullQuantileFunction(
            feature_net,
            space_info.action_info.action_shape,
            [64, 64, 64],
            64,
            device=device,
        ).to(device)

    with LogStep("Creating optimizer..."):
        optim = torch.optim.Adam(net.parameters(), lr=5e-5)

    with LogStep("Creating fraction network..."):
        fraction_net = FractionProposalNetwork(32, embedding_dim=net.input_dim)

    with LogStep("Creating fraction optimiser..."):
        fraction_optim = torch.optim.Adam(fraction_net.parameters(), lr=2.5e-9)

    with LogStep("Creating FQF policy..."):
        policy = FQFPolicy(
            model=net,
            optim=optim,
            fraction_model=fraction_net,
            fraction_optim=fraction_optim,
            action_space=env._action_space,
            discount_factor=0.95,
            num_fractions=128,
            ent_coef=10.0,
            estimation_step=3,
            target_update_freq=320,
        ).to(device)

    # buffer
    with LogStep("Creating replay buffer..."):
        buf = PrioritizedVectorReplayBuffer(
            total_size=20000,
            buffer_num=env.num_envs,
            alpha=0.6,
            beta=0.4,
        )

    with LogStep("Creating collector..."):
        collector = Collector(policy, env, buf, exploration_noise=True)

    batch_size = 64
    collector.collect(n_step=batch_size * env.num_envs)

    with LogStep("Creating tensorboard logger..."):
        log_path = Path(__file__).parent.parent.parent.parent / "logs" / "dqn"
        writer = SummaryWriter(log_path)
        logger = TensorboardLogger(writer)

    def save_best_fn(policy: BasePolicy) -> None:
        torch.save(policy.state_dict(), log_path / "policy.pth")

    def stop_fn(mean_rewards: float) -> bool:
        return False

    def train_fn(epoch: int, env_step: int) -> None:
        # eps annnealing, just a demo
        eps = 0.7
        if env_step <= 100000:
            policy.set_eps(eps)
        elif env_step <= 500000:
            # policy.set_eps(0)
            eps = eps - (env_step - 10000) / 40000 * (0.9 * eps)
            policy.set_eps(eps)
        else:
            # policy.set_eps(0.1 * eps)
            policy.set_eps(0)

    def test_fn(epoch: int, env_step: int | None) -> None:
        eps = 0.05
        policy.set_eps(eps)

    # trainer
    with LogStep("Creating trainer..."):
        trainer = OffpolicyTrainer(
            policy=policy,
            train_collector=collector,
            max_epoch=2000,
            step_per_epoch=10000,
            step_per_collect=30,
            episode_per_test=env.num_envs,
            batch_size=batch_size,
            update_per_step=0.1,
            train_fn=train_fn,
            stop_fn=stop_fn,
            save_best_fn=save_best_fn,
            logger=logger,
        )

    result = trainer.run()

    # if __name__ == "__main__":
    #     pprint.pprint(result)
    #     # Let's watch its performance!
    #     env = gym.make(args.task)
    #     policy.eval()
    #     policy.set_eps(args.eps_test)
    #     collector = Collector(policy, env)
    #     collector_stats = collector.collect(n_episode=1, render=args.render)
    #     print(collector_stats)
