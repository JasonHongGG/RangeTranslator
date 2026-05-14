from __future__ import annotations

import os
import site
import sysconfig
from pathlib import Path


def configure_process_environment() -> None:
    for directory in _candidate_nvidia_bin_dirs():
        _prepend_path(directory)
        _add_dll_directory(directory)


def _candidate_nvidia_bin_dirs() -> list[Path]:
    roots: list[Path] = []
    for candidate in site.getsitepackages():
        roots.append(Path(candidate))

    usersite = site.getusersitepackages()
    if usersite:
        roots.append(Path(usersite))

    platlib = sysconfig.get_path("platlib")
    if platlib:
        roots.append(Path(platlib))

    seen: set[str] = set()
    directories: list[Path] = []
    for root in roots:
        for relative in (
            Path("nvidia/cudnn/bin"),
            Path("nvidia/cublas/bin"),
            Path("nvidia/cuda_runtime/bin"),
            Path("nvidia/cuda_nvrtc/bin"),
        ):
            candidate = (root / relative).resolve()
            key = str(candidate).lower()
            if candidate.exists() and key not in seen:
                seen.add(key)
                directories.append(candidate)

    return directories


def _prepend_path(directory: Path) -> None:
    existing = os.environ.get("PATH", "")
    parts = [part for part in existing.split(os.pathsep) if part]
    normalized = {part.lower() for part in parts}
    value = str(directory)
    if value.lower() in normalized:
        return
    os.environ["PATH"] = os.pathsep.join([value, *parts])


def _add_dll_directory(directory: Path) -> None:
    add_dll_directory = getattr(os, "add_dll_directory", None)
    if add_dll_directory is None:
        return

    try:
        add_dll_directory(str(directory))
    except OSError:
        pass