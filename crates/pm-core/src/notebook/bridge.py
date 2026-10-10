"""Translate line-delimited editor commands to Jupyter channels on one worktree."""

import json
import os
from pathlib import Path
import queue
import sys
import threading

from jupyter_client import KernelManager
from jupyter_client.kernelspec import KernelSpecManager


def emit(kind, **values):
    """Write one protocol event without mixing kernel output into the transport."""
    print(json.dumps(dict(type=kind, **values)), flush=True)


def read_commands(commands):
    """Read commands independently so interrupt remains available during execution."""
    try:
        for line in sys.stdin:
            commands.put(json.loads(line))
    finally:
        commands.put(dict(action="quit"))


def stop(manager, client):
    """Stop the kernel and its channels, including partially started kernels."""
    try:
        if manager is not None and manager.has_kernel:
            manager.shutdown_kernel(now=True)
    finally:
        if client is not None:
            client.stop_channels()


def main():
    """Discover kernels without starting one and handle explicit lifecycle commands."""
    commands = queue.Queue()
    threading.Thread(target=read_commands, args=(commands,), daemon=True).start()
    cache = Path.home() / ".cache" / "pandemonium" / "kernels"
    cache.mkdir(parents=True, exist_ok=True)
    os.environ["TMPDIR"] = str(cache)
    os.environ["TEMP"] = str(cache)
    os.environ["TMP"] = str(cache)
    manager = None
    client = None
    requests = {}
    try:
        while True:
            try:
                command = commands.get(timeout=0.02)
            except queue.Empty:
                command = None
            if command is not None:
                action = command["action"]
                try:
                    if action == "quit":
                        break
                    if action == "discover":
                        specs = KernelSpecManager().get_all_specs()
                        emit("kernels", kernels=[dict(name=name, display_name=spec["spec"]["display_name"], language=spec["spec"]["language"]) for name, spec in specs.items()])
                    elif action in ("start", "restart"):
                        emit("state", state="starting")
                        stop(manager, client)
                        manager = None
                        client = None
                        requests.clear()
                        manager = KernelManager(kernel_name=command["name"])
                        manager.connection_file = str(cache / ("kernel-" + manager.session.session + ".json"))
                        manager.start_kernel(cwd=os.getcwd(), stdout=sys.stderr, stderr=sys.stderr)
                        client = manager.blocking_client()
                        client.start_channels()
                        client.wait_for_ready(timeout=30)
                        emit("state", state="idle")
                    elif action == "shutdown":
                        stop(manager, client)
                        manager = None
                        client = None
                        requests.clear()
                        emit("state", state="stopped")
                    elif action == "interrupt":
                        if manager is not None:
                            manager.interrupt_kernel()
                    elif action == "execute":
                        if client is None:
                            raise RuntimeError("Start a selected kernel before executing cells")
                        for cell in command["cells"]:
                            request = client.execute(cell["source"], allow_stdin=False, stop_on_error=False)
                            requests[request] = cell["id"]
                    else:
                        raise RuntimeError("Unknown notebook command: " + action)
                except Exception as error:
                    emit("failure", message=str(error))
                    stop(manager, client)
                    manager = None
                    client = None
                    requests.clear()
            if client is not None:
                if not manager.is_alive():
                    emit("failure", message="Kernel exited. Restart it to continue; your cells remain editable.")
                    stop(manager, client)
                    manager = None
                    client = None
                    requests.clear()
                    continue
                for _ in range(128):
                    try:
                        message = client.get_iopub_msg(timeout=0)
                    except queue.Empty:
                        break
                    kind = message["header"]["msg_type"]
                    parent = message["parent_header"].get("msg_id")
                    cell = requests.get(parent)
                    content = message["content"]
                    if kind == "update_display_data" or cell is not None:
                        emit(kind, cell=cell, content=content)
                    if kind == "status" and cell is not None:
                        emit("state", state=content["execution_state"])
                        if content["execution_state"] == "idle":
                            emit("done", cell=cell)
                            requests.pop(parent, None)
                for _ in range(128):
                    try:
                        message = client.get_shell_msg(timeout=0)
                    except queue.Empty:
                        break
                    parent = message["parent_header"].get("msg_id")
                    if message["header"]["msg_type"] == "execute_reply" and message["content"].get("status") == "aborted":
                        emit("done", cell=requests.get(parent))
    finally:
        stop(manager, client)


if __name__ == "__main__":
    main()
