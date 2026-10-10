"""Fetch pinned, checksum-verified proxy executables into a disposable test directory."""
import argparse
import hashlib
from pathlib import Path
import platform
import shutil
import tarfile
import tempfile
import urllib.request

PINS = [('caddy', 'caddyserver/caddy', '2.11.7', '512'), ('traefik', 'traefik/traefik', '3.7.14', '256')]


def fetch(url, destination, limit):
    with urllib.request.urlopen(url, timeout=30) as response, destination.open('xb') as output:
        total = 0
        while chunk := response.read(65536):
            total += len(chunk)
            if total > limit:
                raise ValueError('Download exceeds the test artifact budget')
            output.write(chunk)


def install(directory):
    system = {'Linux': 'linux', 'Darwin': 'darwin'}[platform.system()]
    arch = {'x86_64': 'amd64', 'aarch64': 'arm64', 'arm64': 'arm64'}[platform.machine()]
    directory.mkdir(parents=True, exist_ok=True)
    for name, repo, version, algorithm in PINS:
        if (directory / name).exists() or (directory / name).is_symlink():
            raise ValueError('Use an empty disposable directory; existing executables are preserved')
        archive_name = f'{name}_{"v" if name == "traefik" else ""}{version}_{system}_{arch}.tar.gz'
        checksum_name = f'{name}_{"v" if name == "traefik" else ""}{version}_checksums.txt'
        base = f'https://github.com/{repo}/releases/download/v{version}'
        with tempfile.TemporaryDirectory(prefix='fetch-', dir=directory) as scratch:
            scratch = Path(scratch)
            catalog, archive = scratch / 'checksums', scratch / 'archive'
            fetch(base + '/' + checksum_name, catalog, 65536)
            rows = [line.split()[0] for line in catalog.read_text().splitlines() if line.split()[1:] == [archive_name]]
            if len(rows) != 1:
                raise ValueError('Missing or ambiguous upstream checksum')
            fetch(base + '/' + archive_name, archive, 100 * 1024 * 1024)
            digest = hashlib.new('sha' + algorithm)
            with archive.open('rb') as source:
                for chunk in iter(lambda: source.read(65536), b''):
                    digest.update(chunk)
            if digest.hexdigest() != rows[0]:
                raise ValueError('Upstream checksum mismatch')
            output = scratch / name
            with tarfile.open(archive, 'r:gz') as bundle:
                member = bundle.getmember(name)
                if not member.isfile() or member.size > 256 * 1024 * 1024:
                    raise ValueError('Invalid or oversized executable member')
                with bundle.extractfile(member) as source, output.open('xb') as target:
                    shutil.copyfileobj(source, target, length=65536)
            output.chmod(0o755)
            output.replace(directory / name)
        print(f'{name} {version}: {system}/{arch}, SHA{algorithm} verified')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dir', type=Path, required=True)
    args = parser.parse_args()
    install(args.dir)
