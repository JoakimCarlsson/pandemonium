"""Discover unittest cases and journal results for the editor's task runner."""

import contextlib
import inspect
import io
import json
from pathlib import Path
import sys
import time
import traceback
import unittest


def flatten(suite):
    """Yield individual cases from nested framework suites."""
    for test in suite:
        if isinstance(test, unittest.TestSuite):
            yield from flatten(test)
        else:
            yield test


def location(test):
    """Find the source definition of a test method when available."""
    try:
        method = getattr(type(test), test._testMethodName)
        path = Path(inspect.getfile(method)).resolve()
        if not path.is_relative_to(Path.cwd().resolve()):
            return None
        return {"path": str(path), "line": inspect.getsourcelines(method)[1] - 1}
    except (AttributeError, TypeError, OSError):
        return None


def case(test):
    """Normalize a framework identity into the editor's case schema."""
    identity = test.id()
    return {"id": identity, "suite": identity.rsplit(".", 1)[0],
            "location": location(test), "status": "unknown",
            "duration": 0, "started": 0, "output": "", "failure": None}


class Journal(unittest.TestResult):
    """Persist every state transition before the process can be cancelled."""

    def __init__(self, destination, tests):
        """Initialize the output journal and result accumulator."""
        super().__init__()
        self.destination = destination
        self.record = None
        self.started = 0
        self.capture = None
        self.redirect = None
        self.active = None
        self.tests = tests
        self.completed = set()

    def write(self, record):
        """Append a flushed JSON record without depending on terminal history."""
        with self.destination.open("a", encoding="utf-8") as output:
            output.write(json.dumps(record) + "\n")
            output.flush()
        print(record["id"] + " · " + record["status"], file=sys.__stdout__, flush=True)

    def startTest(self, test):
        """Record a started case and capture its stdout and stderr."""
        super().startTest(test)
        self.active = test
        self.record = case(test)
        self.record["status"] = "running"
        self.started = time.monotonic()
        self.record["started"] = time.time()
        self.write(self.record)
        self.capture = LiveCapture(self.destination.parent / "active-output.txt")
        self.redirect = contextlib.ExitStack()
        self.redirect.enter_context(contextlib.redirect_stdout(self.capture))
        self.redirect.enter_context(contextlib.redirect_stderr(self.capture))

    def stopTest(self, test):
        """Keep output and elapsed time for every completed case."""
        self.redirect.close()
        self.record["output"] += self.capture.getvalue()
        self.capture.close()
        self.record["duration"] = time.monotonic() - self.started
        self.write(self.record)
        self.active = None
        self.completed.add(test.id())
        super().stopTest(test)

    def addSuccess(self, test):
        """Mark a normally completed case as passed."""
        super().addSuccess(test)
        if self.record["status"] != "failed":
            self.record["status"] = "passed"

    def failure(self, test, error):
        """Retain a traceback and its innermost location in this worktree."""
        record = self.record if test is self.active else case(test)
        record["status"] = "failed"
        record["output"] += "".join(traceback.format_exception(*error))
        root = Path.cwd().resolve()
        for frame in traceback.extract_tb(error[2]):
            path = Path(frame.filename).resolve()
            if path.is_relative_to(root):
                record["failure"] = {"path": str(path), "line": frame.lineno - 1}
        if test is not self.active:
            self.write(record)
            self.fixture(test, "failed", record["output"], record["failure"])

    def fixture(self, test, status, output, failure=None):
        """Apply failed or skipped class/module setup to unexecuted member cases."""
        identity = test.id()
        if " (" not in identity or not identity.endswith(")"):
            return
        prefix = identity.split(" (", 1)[1][:-1] + "."
        for member in self.tests:
            if member.id().startswith(prefix) and member.id() not in self.completed:
                record = case(member)
                record.update(status=status, output=output, failure=failure)
                self.write(record)
                self.completed.add(member.id())

    def addFailure(self, test, error):
        """Keep an assertion failure as a failed case."""
        super().addFailure(test, error)
        self.failure(test, error)

    def addError(self, test, error):
        """Keep setup, teardown and discovery errors as failures."""
        super().addError(test, error)
        self.failure(test, error)

    def addSkip(self, test, reason):
        """Record an explicit skip and its reason."""
        super().addSkip(test, reason)
        record = self.record if test is self.active else case(test)
        record["status"] = "skipped"
        record["output"] += reason + "\n"
        if test is not self.active:
            self.write(record)
            self.fixture(test, "skipped", reason)

    def addExpectedFailure(self, test, error):
        """Show expected failures as skipped with their traceback."""
        super().addExpectedFailure(test, error)
        self.record["status"] = "skipped"
        self.record["output"] += "Expected failure\n" + "".join(traceback.format_exception(*error))

    def addUnexpectedSuccess(self, test):
        """Treat an unexpected success as a failed expectation."""
        super().addUnexpectedSuccess(test)
        self.record["status"] = "failed"
        self.record["output"] += "Unexpected success\n"

    def addSubTest(self, test, subtest, error):
        """Attach subtest failures to their independently selectable parent."""
        super().addSubTest(test, subtest, error)
        if error is not None:
            self.record["output"] += str(subtest) + "\n"
            self.failure(test, error)


class LiveCapture(io.StringIO):
    """Keep interrupted output on disk as well as in the case result."""

    def __init__(self, path):
        """Open a bounded-lifetime live output file for this case."""
        super().__init__()
        self.file = path.open("w", encoding="utf-8")

    def write(self, value):
        """Flush every write so termination preserves useful diagnostics."""
        self.file.write(value)
        self.file.flush()
        sys.__stdout__.write(value)
        sys.__stdout__.flush()
        return super().write(value)

    def close(self):
        """Release both capture destinations."""
        self.file.close()
        super().close()


def main():
    """Execute a discovery or selection plan in the task's own worktree."""
    plan_path = Path(sys.argv[1])
    plan = json.loads(plan_path.read_text())
    config = plan["adapter"]
    root = Path.cwd().resolve()
    top = root / config["top"]
    sys.path.insert(0, str(top))
    coverage = None
    if plan["coverage"]:
        import coverage as coverage_module
        coverage = coverage_module.Coverage(source=[str(root)], data_file=str(plan_path.parent / ".coverage"))
        coverage.start()
    loader = unittest.TestLoader()
    suite = loader.discover(str(root / config["start"]), config["pattern"], str(top))
    tests = list(flatten(suite))
    (plan_path.parent / "discovery.json").write_text(json.dumps([case(test) for test in tests]))
    if loader.errors:
        raise RuntimeError("\n".join(loader.errors))
    selection = plan["selection"]
    if selection is None:
        return
    if isinstance(selection, dict):
        kind, identity = next(iter(selection.items()))
        tests = [test for test in tests if (test.id() if kind == "case" else case(test)["suite"]) == identity]
        if not tests:
            raise RuntimeError("Selected test no longer exists: " + identity)
        suite = unittest.TestSuite(tests)
    result = Journal(plan_path.parent / "results.jsonl", tests)
    try:
        suite.run(result)
    finally:
        if coverage is not None:
            coverage.stop()
            coverage.save()
            coverage.lcov_report(outfile=str(plan_path.parent / "coverage.lcov"))
    print(f"{result.testsRun} tests; {len(result.failures)} failures; {len(result.errors)} errors; {len(result.skipped)} skipped")
    if not result.wasSuccessful():
        sys.exit(1)


if __name__ == "__main__":
    main()
