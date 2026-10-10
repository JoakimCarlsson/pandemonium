# Test explorer

Open **Open Test Explorer** in the command palette, or **Tests** in the tools
menu. Its ordinary pane can be split, moved, tabbed and closed. Projects and
worktrees have separate discoveries, runs, results and coverage. Each Python
unittest class forms a suite; clicking a case opens its failure location or its
source definition and selects its retained output below the inventory.

**Refresh** discovers `test*.py` from the worktree root. Saving Python files,
changing the discovery configuration or changing the repository revision
schedules discovery again after the current run finishes. Unsaved source must
be saved before discovery or execution reads it.

**Run all**, **Run suite** and **Run** execute the corresponding selection.
The inventory retains the latest result for each case; clicking a numbered run
shows that run's exact subset and results. **Refresh** returns to the inventory.
Passed, failed, skipped, queued, running and cancelled are distinct states.
Expected failures are skipped with their traceback; unexpected successes fail;
subtest failures attach to their selectable parent. Durations are in seconds.
**Cancel** stops the current discovery or execution through the normal task
runner. Flushed case output survives interruption. **Terminal output** opens
the retained task shell, including discovery, infrastructure and fixture errors.
Runs stay independent of ordinary project tasks and health check tasks.

## Python configuration

The standard-library unittest adapter needs Python 3.9 or newer. Configure its
interpreter and discovery in the worktree's optional `.pandemonium/tests.json`:

```json
{
  "interpreter": ".venv/bin/python",
  "start": "tests",
  "top": ".",
  "pattern": "test*.py"
}
```

Defaults are `python3`, `.`, `.`, and `test*.py`. Directories must remain within
the worktree. Follow unittest's importable-module and package requirements;
use the same import root for discovery and debugging. Discovery imports tests,
including a package's `load_tests` hook, as unittest normally does. Syntax or
import errors are shown as discovery errors rather than an empty successful run.

**Debug** launches one case with the existing debugpy adapter and the configured
interpreter. Install `debugpy` into the Python used to start the editor's debugpy
adapter (the `python3` or `python` found on its PATH). The target interpreter must
also support debugpy. Set a normal editor gutter breakpoint in that case, then
click **Debug**. The existing debugger pane supplies continue, stepping, stack,
variables, watches and stop. Cases synthesized by custom `load_tests` hooks must
be individually importable by their unittest identity for debugging. Adapter
startup or unsupported identity errors are reported by the debugger.

## Coverage

**Collect coverage** runs all tests with coverage.py installed in the configured
interpreter (`python -m pip install coverage`). It records imports and test
execution, exports LCOV and shows covered/executable counts for each source
file. Failed assertions still produce coverage; an interrupted run without a
report or a missing coverage.py dependency reports an error. Subprocess coverage
requires the project's own coverage.py subprocess configuration.

**Import coverage.lcov** reads an existing LCOV report at the worktree root. For
example, generate one with `python -m coverage run --source=. -m unittest` followed
by `python -m coverage lcov -o coverage.lcov`. Imported reports are explicitly
attached to the current saved source revision: generate the report from that
revision before importing it. LCOV itself carries no complete source revision.
Files outside the worktree and malformed source/line records are rejected.

Click a summary to open its first uncovered line. Covered executable lines have
a green gutter mark; uncovered lines have a red mark and a faint red wash.
Collected coverage captures the saved Python sources before execution. Changes
during execution invalidate it. Source edits, dirty buffers and repository
revision changes mark coverage **STALE** and hide its decorations. **Clear
coverage** removes summaries and marks. Reports never cross worktree boundaries.

## Adding a framework adapter

Framework concerns live in `pm-core/src/testing`, independent of presentation.
Add an adapter module there and re-export its public API from `mod.rs`:

1. Produce `Case` identities with a suite and zero-based source `Location`.
   Identities are unique within a `Scope`, never global across worktrees.
2. Return a `Task` execution plan for discovery and all/suite/case selections;
   use the existing `App::run_task` and `Tasks` lifecycle. Do not spawn processes
   from the adapter, explorer or result store.
3. Normalize per-case transitions, output, duration and failure locations into
   `Case` records. Persist records and active output as they happen so killed
   processes do not lose earlier results. Include fixture and infrastructure
   failures and retain the task outcome separately from case outcomes.
4. Supply a `pm-dap::Scenario` only for supported single-case debugging, through
   `App::start_debugging`. Explicitly report missing capabilities.
5. Export LCOV for supported coverage collection or expose report import.
   Capture the source revision before execution and invalidate stale reports.
6. Dispatch the adapter in `pandemonium/src/app/testing.rs`; retain all discovery,
   task IDs, results and coverage under the full project/worktree `Scope` in
   `pandemonium/src/testing/store.rs`. The shared pane consumes normalized data.

The Python reference adapter uses a private task journal under
`$XDG_CACHE_HOME/pandemonium/tests` (otherwise `~/.cache/pandemonium/tests`).
Journals are removed when their worktree leaves the editor or the editor exits;
no build or task artifacts are placed in `/tmp` or the tested worktree.
