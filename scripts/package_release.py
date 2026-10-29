"""Package one explicitly built native target; never guesses or overwrites output."""
import argparse
import hashlib
import pathlib
import re
import tarfile
import tomllib

from prepare_installer import TARGETS

BUNDLE_FILES = (
    "README.md", "docs/AGENT_GUIDE.md", "llms.txt",
    "examples/read-only.yaml", "examples/routes-only.yaml", "examples/caddy-http.json",
    "examples/service-stack.json", "examples/static-server.py",
    "fixtures/caddy/snapshot.json", "fixtures/traefik/snapshot.json",
)


def package(version, target, output):
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version) or target not in TARGETS.values():
        raise ValueError("invalid release version or target")
    metadata = tomllib.loads(pathlib.Path("Cargo.toml").read_text())
    if version != "v" + metadata["package"]["version"]:
        raise ValueError("tag must match the package version")
    binary = pathlib.Path("target") / target / "release" / "steward"
    if not binary.is_file() or binary.is_symlink():
        raise ValueError("build the requested target before packaging")
    for name in BUNDLE_FILES:
        source = pathlib.Path(name)
        if not source.is_file() or source.is_symlink():
            raise ValueError("missing regular release input: " + name)
    output.mkdir(parents=True, exist_ok=True)
    name = f"steward_{version}_{target}.tar.gz"
    archive = output / name
    sidecar = archive.with_name(name + ".sha256")
    if archive.exists() or archive.is_symlink() or sidecar.exists() or sidecar.is_symlink():
        raise ValueError("archive/checksum already exists; use a fresh output directory")
    with tarfile.open(archive, "x:gz") as tar:
        tar.add(binary, arcname="steward", recursive=False)
        for name in BUNDLE_FILES:
            tar.add(name, arcname=name, recursive=False)
    with archive.open("rb") as file:
        digest = hashlib.file_digest(file, "sha256").hexdigest()
    with sidecar.open("x") as file:
        file.write(f"{digest}  {archive.name}\n")
    print(archive)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", required=True, choices=sorted(TARGETS.values()))
    parser.add_argument("--output", type=pathlib.Path, default=pathlib.Path("release-dist"))
    args = parser.parse_args()
    try:
        package(args.version, args.target, args.output)
    except ValueError as error:
        parser.exit(1, str(error) + "\n")
