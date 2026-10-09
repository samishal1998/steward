"""Package one explicitly built native target; never guesses or overwrites output."""
import argparse
import hashlib
import pathlib
import re
import tarfile
import tomllib

from prepare_installer import TARGETS


def package(version, target, output):
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version) or target not in TARGETS.values():
        raise ValueError("invalid release version or target")
    metadata = tomllib.loads(pathlib.Path("Cargo.toml").read_text())
    if version != "v" + metadata["package"]["version"]:
        raise ValueError("tag must match the package version")
    binary = pathlib.Path("target") / target / "release" / "steward"
    if not binary.is_file() or binary.is_symlink():
        raise ValueError("build the requested target before packaging")
    output.mkdir(parents=True, exist_ok=True)
    name = f"steward_{version}_{target}.tar.gz"
    archive = output / name
    if archive.exists():
        raise ValueError("archive already exists; use a fresh output directory")
    with tarfile.open(archive, "x:gz") as tar:
        tar.add(binary, arcname="steward", recursive=False)
        tar.add("README.md", arcname="README.md", recursive=False)
    with archive.open("rb") as file:
        digest = hashlib.file_digest(file, "sha256").hexdigest()
    archive.with_name(name + ".sha256").write_text(f"{digest}  {name}\n")
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
