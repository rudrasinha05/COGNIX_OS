"""Persistent BDR task runner. No AI API, paid services or arbitrary shell execution."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "tools" / "bdr_tasks.json"
STATE = ROOT / ".bdr-state.json"

def load_tasks(path):
    tasks = json.loads(path.read_text(encoding="utf-8"))
    ids = {t["id"] for t in tasks}
    if len(ids) != len(tasks):
        raise ValueError("Duplicate task ID")
    for t in tasks:
        if t["id"] in t["deps"] or not set(t["deps"]) <= ids:
            raise ValueError("Invalid dependency")
    remaining = set(ids)
    while remaining:
        ready = {t["id"] for t in tasks if t["id"] in remaining and not set(t["deps"]) & remaining}
        if not ready:
            raise ValueError("Dependency cycle")
        remaining -= ready
    return tasks

def save(path, state):
    fd, name = tempfile.mkstemp(dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(state, f, indent=2, sort_keys=True)
            f.flush()
            os.fsync(f.fileno())
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)

def command(argv):
    if not isinstance(argv, list) or not argv or not all(isinstance(x, str) for x in argv):
        raise ValueError("Expected an explicit argument vector")
    try:
        return subprocess.run(argv, cwd=ROOT, shell=False, timeout=600).returncode == 0
    except (OSError, subprocess.TimeoutExpired) as exc:
        print(str(exc), file=sys.stderr)
        return False

def run(tasks, state, persist, execute):
    # Reverify on each invocation: old success is not evidence for changed source.
    done = set()
    attempted = set()
    while True:
        ready = [t for t in tasks if t["id"] not in attempted and set(t["deps"]) <= done
                 and not t.get("owner_required", False) and t.get("verify")]
        if not ready:
            break
        for t in ready:
            key = t["id"]
            attempted.add(key)
            state[key] = "running"
            persist(state)
            ok = (not t.get("implement") or execute(t["implement"])) and execute(t["verify"])
            state[key] = "verified" if ok else "failed"
            if ok:
                done.add(key)
            persist(state)
    for t in tasks:
        key = t["id"]
        if key not in attempted:
            state[key] = "owner_required" if t.get("owner_required") else (
                "awaiting_implementation" if not t.get("verify") else "dependency_blocked")
    persist(state)
    return 0 if len(done) == len(tasks) else 2

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--audit", action="store_true")
    args = p.parse_args()
    tasks = load_tasks(MANIFEST)
    if args.audit:
        for t in tasks:
            print(t["id"], "owner" if t.get("owner_required") else "independent",
                  "runnable" if t.get("verify") else "needs developer implementation")
        return 0
    branch = subprocess.check_output(["git", "branch", "--show-current"], cwd=ROOT, text=True).strip()
    if branch != "develop":
        print("BDR refuses execution outside develop", file=sys.stderr)
        return 3
    state = {}
    result = run(tasks, state, lambda s: save(STATE, s), command)
    print(json.dumps(state, indent=2))
    return result

if __name__ == "__main__":
    sys.exit(main())
