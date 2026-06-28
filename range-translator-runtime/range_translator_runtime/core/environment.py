from __future__ import annotations

import os
import site
import sysconfig
import logging
import warnings
from pathlib import Path


def configure_process_environment() -> None:
    _configure_runtime_cache()
    for directory in _candidate_nvidia_bin_dirs():
        _prepend_path(directory)
        _add_dll_directory(directory)

    os.environ.setdefault("PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK", "True")
    
    # 關閉 Paddle C++ 核心日誌，減少控制台噪音
    os.environ.setdefault("GLOG_minloglevel", "2")
    os.environ.setdefault("GLOG_v", "0")
    
    # 阻斷 Paddle JIT C++ 編譯，強制使用純 Python 替代方案，解決啟動卡頓問題
    _block_paddle_jit_compilation()


def _block_paddle_jit_compilation() -> None:
    """Monkey-patch paddle.utils.cpp_extension to raise an error and abort compilation immediately"""
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            import paddle.utils.cpp_extension
            
        def _mock_setup(*args, **kwargs):
            logging.debug("Paddle JIT compilation blocked by RangeTranslator to improve startup speed.")
            raise ImportError("JIT compilation intentionally blocked to force Python fallback")
            
        paddle.utils.cpp_extension.setup = _mock_setup
    except ImportError:
        pass


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
            Path("nvidia/cufft/bin"),
            Path("nvidia/curand/bin"),
            Path("nvidia/cusolver/bin"),
            Path("nvidia/cusparse/bin"),
            Path("nvidia/nvjitlink/bin"),
        ):
            candidate = (root / relative).resolve()
            key = str(candidate).lower()
            if candidate.exists() and key not in seen:
                seen.add(key)
                directories.append(candidate)

    return directories


def _configure_runtime_cache() -> None:
    runtime_root = Path(__file__).resolve().parents[1]
    cache_dir = runtime_root / ".runtime" / "paddlex"
    cache_dir.mkdir(parents=True, exist_ok=True)
    os.environ.setdefault("PADDLE_PDX_CACHE_HOME", str(cache_dir))


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