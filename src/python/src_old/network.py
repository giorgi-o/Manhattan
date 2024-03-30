from dataclasses import dataclass
from datetime import datetime
import base64
import json
import os
from pathlib import Path
from typing import cast, TYPE_CHECKING, Any, Iterable

import torch
from torch import nn
import numpy as np


# prevent circular import
if TYPE_CHECKING:
    from environment.environment import State, Environment, Action
    from dqn.dqn import TransitionBatch, TdTargetBatch
else:
    Experience = object
    State = object
    Action = object
    Environment = object
    TransitionBatch = object
    TdTargetBatch = object


class TorchJSONEncoder(json.JSONEncoder):
    """
    Custom JSON encoder for torch tensors, used in the custom save() method of our ActorModule.
    """

    def default(self, obj):
        if isinstance(obj, torch.Tensor):
            return obj.cpu().detach().numpy().tolist()
        return json.JSONEncoder.default(self, obj)


class TorchJSONDecoder(json.JSONDecoder):
    """
    Custom JSON decoder for torch tensors, used in the custom load() method of our ActorModule.
    """

    def __init__(self, *args, **kwargs):
        super().__init__(object_hook=self.object_hook, *args, **kwargs)

    def object_hook(self, dct):
        for key in dct.keys():
            if isinstance(dct[key], list):
                dct[key] = torch.Tensor(dct[key])
        return dct


class NeuralNetwork(nn.Module):
    @staticmethod
    def device() -> torch.device:
        """Utility function to determine whether we can run on GPU"""
        device = (
            "cuda" if torch.cuda.is_available() else "mps" if torch.backends.mps.is_available() else "cpu"
        )
        return torch.device(device)

    @staticmethod
    def tensorify(array: Iterable) -> torch.Tensor:
        """Create a PyTorch tensor, and make sure it's on the GPU if possible"""
        return torch.tensor(array, device=NeuralNetwork.device())

    def __init__(self, inputs: int, outputs: int):
        super(NeuralNetwork, self).__init__()

        self.inputs = inputs
        self.outputs = outputs
        self.stack = self.create_stack()
        self.optim = self.create_optim()

        # move to gpu if possible
        self.to(NeuralNetwork.device())

    def create_copy(self):
        raise NotImplementedError  # need to subclass this

    def copy_from(self, other: "NeuralNetwork"):
        self.load_state_dict(other.state_dict())

    def save_checkpoint(
        self,
        filename: str,
        suffix: str | None = None,
        **kwargs,
    ) -> tuple[Path, str]:
        now = datetime.now()

        foldername = now.strftime("%Y-%m-%d %H.%M")
        if suffix is not None:
            foldername += f" {suffix}"

        folder = Path(__file__).parent / "checkpoints" / foldername
        os.makedirs(folder, exist_ok=True)

        json_filename = Path(__file__).parent / "checkpoints" / foldername / f"{filename}.json"
        txt_filename = Path(__file__).parent / "checkpoints" / foldername / "info.txt"

        with open(json_filename, "w") as json_file:
            json.dump(self.state_dict(), json_file, cls=TorchJSONEncoder)

        info = f"created at: {now.strftime('%Y-%m-%d %H:%M:%S')}\n"
        checkpoint_id = base64.b64encode(foldername.encode("utf-8")).decode("utf-8")
        info += f"id: {checkpoint_id}\n"
        for key, value in kwargs.items():
            info += f"{key}: {value}\n"
        info += f"\n"
        info += f"comments:\n"

        txt_filename.write_text(info)

        return folder, checkpoint_id

    def load_checkpoint(self, b64_id: str, filename: str):
        foldername = base64.b64decode(b64_id.encode("utf-8")).decode("utf-8")
        json_filename = Path(__file__).parent / "checkpoints" / foldername / f"{filename}.json"

        with open(json_filename, "r") as json_file:
            self.load_state_dict(json.load(json_file, cls=TorchJSONDecoder))

        print(f"loaded checkpoint from: {json_filename}")

    def create_stack(self) -> nn.Sequential:
        # default stack, subclasses can override this
        neurons = 128
        return nn.Sequential(
            nn.Linear(self.inputs, neurons),
            nn.ReLU(),
            nn.Linear(neurons, neurons),
            nn.ReLU(),
            nn.Linear(neurons, self.outputs),
        )

    def create_optim(self) -> torch.optim.Optimizer:
        # default optim, subclasses can override this
        return torch.optim.AdamW(self.parameters(), lr=1e-4, amsgrad=True)

    def reset_output_weights(self, range: float = 3e-3):
        # first, find output layer (last nn.Linear in stack)
        output_layer = None
        for layer in reversed(self.stack):
            if isinstance(layer, nn.Linear):
                output_layer = cast(nn.Linear, layer)
                break
        assert output_layer is not None

        output_layer.weight.data.uniform_(-range, range)
        output_layer.bias.data.uniform_(-range, range)

    # do not call directly, call get_q_values() instead
    def forward(self, state: torch.Tensor) -> torch.Tensor:
        """PyTorch internal function to perform forward pass.
        Do not call directly, use get_q_values() instead.

        Args:
            state (torch.Tensor): a tensor of length 6 (the state has 6 variables)

        Returns:
            torch.Tensor: a tensor of length 3 (one q-value for each action)
        """

        return self.stack(state)

    def gradient_descent(self, expected: torch.Tensor, actual: torch.Tensor) -> float:
        criterion = torch.nn.HuberLoss()
        loss = criterion(expected, actual)

        self.optim.zero_grad()
        loss.backward()

        # clip gradients
        nn.utils.clip_grad.clip_grad_value_(self.parameters(), 100.0)

        self.optim.step()  # gradient descent

        return loss.item()

    def polyak_update(self, main: "NeuralNetwork", update_rate: float):
        main_net_state = main.state_dict()
        target_net_state = self.state_dict()
        β = update_rate  # shorten name

        for key in target_net_state:
            target_net_state[key] = β * main_net_state[key] + (1 - β) * target_net_state[key]

        self.load_state_dict(target_net_state)
