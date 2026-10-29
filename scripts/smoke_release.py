"""Exercise a real release binary or complete archive using isolated, offline inputs."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

from package_release import BUNDLE_FILES

ROOT = Path(__file__).resolve().parents[1]


def extract(archive, destination):
    expected = {"steward", *BUNDLE_FILES}
    seen, total = set(), 0
    with tarfile.open(archive, "r:gz") as package:
        for entry in package:
            if entry.name not in expected or entry.name in seen or not entry.isfile():
                raise ValueError("unexpected, duplicate, or non-regular archive member")
            total += entry.size
            if total > 128 * 1024 * 1024:
                raise ValueError("release archive exceeds 128 MiB")
            seen.add(entry.name)
            target = destination / entry.name
            target.parent.mkdir(parents=True, exist_ok=True)
            with package.extractfile(entry) as source, target.open("xb") as output:
                shutil.copyfileobj(source, output, 65536)
    if seen != expected:
        raise ValueError("release archive is missing its executable or documented examples")
    (destination / "steward").chmod(0o755)


def check(binary, version, inputs, working):
    state = working / "state"

    def run(*args, code=0, json_mode=True):
        command = [str(binary), "--state-dir", str(state), *args]
        if json_mode:
            command.append("--json")
        result = subprocess.run(command, cwd=working, capture_output=True, timeout=15)
        if result.returncode != code or result.stderr:
            raise AssertionError((args, result.returncode, result.stdout.decode(), result.stderr.decode()))
        if json_mode:
            value = json.loads(result.stdout)
            assert value["schema_version"] == "1" and value["changed"] is False
            return value
        return result.stdout.decode()

    assert run("--version", json_mode=False).strip() == "steward " + version.removeprefix("v")
    help_text = run("--help", json_mode=False)
    for command in ("proxies", "routes", "services", "operations", "plan", "apply", "up", "down", "tui", "export", "completions"):
        assert command in help_text, command
    for shell in ("bash", "zsh", "fish", "elvish", "powershell"):
        script = run("completions", shell, json_mode=False)
        assert "upstream" in script
        if shell == "bash":
            subprocess.run(["bash", "-n"], input=script, text=True, check=True, timeout=5)
    assert not state.exists(), "help/version/completion must not initialize state"
    for group in ("proxies", "routes", "services", "operations"):
        assert run(group, "list")["data"] == []
    assert run("export")["data"]["kind"] == "steward.inventory"
    profile = inputs / "examples/read-only.yaml"
    intent = inputs / "examples/routes-only.yaml"
    assert len(run("--config", str(profile), "routes", "list")["data"]) == 4
    assert run("--config", str(profile), "plan", "--file", str(intent))["data"]["applicable"] is False
    empty = working / "empty.json"
    empty.write_text(json.dumps({"schema_version": "1", "scope": "user", "routes": [], "services": []}))
    plan = working / "reviewed.json"
    assert run("plan", "--file", str(empty), "--out", str(plan))["data"]["applicable"] is True
    assert run("apply", "--plan", str(plan), "--yes")["data"]["operations"] == []
    assert run("apply", "--plan", str(plan), code=2)["diagnostics"][0]["code"] == "plan.required"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    selection = parser.add_mutually_exclusive_group(required=True)
    selection.add_argument("--binary", type=Path)
    selection.add_argument("--archive", type=Path)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="steward-release-smoke-") as temporary:
        working = Path(temporary)
        if args.archive:
            inputs = working / "package"
            inputs.mkdir()
            extract(args.archive.resolve(), inputs)
            binary = inputs / "steward"
        else:
            inputs, binary = ROOT, args.binary.resolve()
        check(binary, args.version, inputs, working)
    print("Release smoke passed: version, command tree, five completions, isolated state, bundled inventory, checked plan/apply, and export.")
