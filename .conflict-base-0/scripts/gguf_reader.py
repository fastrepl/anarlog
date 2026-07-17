#!/usr/bin/env python3

import os
import sys
import urllib.request

os.environ["NO_LOCAL_GGUF"] = "1"


def download_and_execute_script(url, args):
    try:
        with urllib.request.urlopen(url) as response:
            script_content = response.read().decode("utf-8")

        original_argv = sys.argv
        sys.argv = ["gguf_dump.py"] + args

        script_globals = {
            "__name__": "__main__",
            "__file__": "gguf_dump.py",
        }

        exec(script_content, script_globals)

    except Exception as e:
        print(e)
        sys.exit(1)
    finally:
        sys.argv = original_argv


if __name__ == "__main__":
    script_url = "https://raw.githubusercontent.com/ggml-org/llama.cpp/refs/heads/master/gguf-py/gguf/scripts/gguf_dump.py"
    script_args = sys.argv[1:] + ["--no-tensors"]
    download_and_execute_script(script_url, script_args)
