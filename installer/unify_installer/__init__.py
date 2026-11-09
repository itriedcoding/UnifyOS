"""UnifyOS installer.

The package is split so the dangerous parts stay reviewable:

* :mod:`unify_installer.plan` — validated, side-effect-free installation plan
* :mod:`unify_installer.disk` — block-device discovery and safety checks
* :mod:`unify_installer.install` — executes a plan against a real disk
* :mod:`unify_installer.cli` — command-line front end (GUI front end not yet
  implemented; see documentation/limitations.md)
"""

from .plan import InstallPlan, PlanError, auto_plan, partition_layout, swap_size_mib
from .disk import BlockDevice, SafetyError, check_install_target, list_block_devices
from .install import Installer, InstallError

__all__ = [
    "BlockDevice",
    "InstallError",
    "InstallPlan",
    "Installer",
    "PlanError",
    "SafetyError",
    "auto_plan",
    "check_install_target",
    "list_block_devices",
    "partition_layout",
    "swap_size_mib",
]

__version__ = "0.1.0"
