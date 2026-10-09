"""Check all four archive digests, then create the release checksum catalog."""
import argparse
import hashlib
import pathlib
import re

from prepare_installer import TARGETS


def catalog(directory, version, verify=None):
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError("invalid release tag")
    records = {}
    for target in sorted(TARGETS.values()):
        name = f"steward_{version}_{target}.tar.gz"
        archive = directory / name
        with archive.open("rb") as file:
            digest = hashlib.file_digest(file, "sha256").hexdigest()
        if archive.with_name(name + ".sha256").read_text().split() != [digest, name]:
            raise ValueError("artifact sidecar does not match " + name)
        records[name] = digest
    if verify:
        published = {}
        for line in verify.read_text().splitlines():
            pieces = line.split()
            if len(pieces) != 2 or pieces[1] in published:
                raise ValueError("invalid or duplicate published checksum")
            published[pieces[1]] = pieces[0]
        if any(published.get(name) != digest for name, digest in records.items()):
            raise ValueError("published binary digests differ; refusing to overwrite an immutable release")
    installer = directory / "install.sh"
    if installer.is_file():
        records["install.sh"] = hashlib.sha256(installer.read_bytes()).hexdigest()
    (directory / "SHA256SUMS").write_text("".join(f"{digest}  {name}\n" for name, digest in sorted(records.items())))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dir", type=pathlib.Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--verify", type=pathlib.Path)
    args = parser.parse_args()
    try:
        catalog(args.dir, args.version, args.verify)
    except ValueError as error:
        parser.exit(1, str(error) + "\n")
