#!/usr/bin/env python3

# https://github.com/thewh1teagle/vibe/blob/9ffde8a/scripts/pre_build.js

import os
import sys
import json
import subprocess
import urllib.request
import shutil
import platform
from pathlib import Path

# BLAS
# https://github.com/utilityai/llama-cpp-rs/blob/2f433cd/llama-cpp-sys-2/build.rs#L279-L281

# OPENMP
# Can cause `the code execution cannot proceed because VCOMP140.DLL was not found` error on Windows


exports = {
    "windows_libclang": r"C:\Program Files\LLVM\bin",
    "windows_cmake": r"C:\Program Files\Cmake\bin",
}


class JsonMidifier:
    def __init__(self, path: str):
        self.path = path
        self.content = {}

    def __enter__(self) -> "JsonMidifier":
        with open(self.path, "r") as f:
            self.content = json.load(f)
        return self

    def __exit__(self, exc_type: type, exc_value: Exception, traceback: object):
        with open(self.path, "w") as f:
            json.dump(self.content, f, indent=4)


def is_windows():
    return platform.system() == "Windows"


def is_macos():
    return platform.system() == "Darwin"


def has_feature(name: str) -> bool:
    return f"--{name}" in sys.argv or name in sys.argv


def setup_vulkan():
    if not is_windows():
        return


def run_cmd(args):
    subprocess.run(args, check=True, capture_output=True, text=True)


def main():
    script_dir = Path(__file__).parent
    tauri_dir = script_dir.parent / "apps" / "desktop" / "src-tauri"
    os.chdir(tauri_dir)
    print(f"chdir: '{tauri_dir}'")

    # with JsonMidifier("./tauri.conf.json") as tauri:
    #     tauri.content["a"] = {"b": 1}

    if has_feature("vulkan"):
        setup_vulkan()


if __name__ == "__main__":
    main()
