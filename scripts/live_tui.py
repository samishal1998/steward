"""Exercise visible TUI screens with a disposable synthetic API and a bounded PTY."""
import argparse
import codecs
import fcntl
import http.server
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import socketserver
import struct
import subprocess
import tempfile
import termios
import threading
import time
import unicodedata


class Screen:
    """Minimal VT screen, so assertions cannot pass on text from an obsolete frame."""
    def __init__(self, width, height):
        self.decoder = codecs.getincrementaldecoder('utf-8')('replace')
        self.pending = ''
        self.resize(width, height)

    def resize(self, width, height):
        self.width, self.height = width, height
        self.cells = [[' '] * width for _ in range(height)]
        self.x = self.y = 0
        self.wrapped = False
        self.complete = True

    def feed(self, data):
        self.pending += self.decoder.decode(data)
        while self.pending:
            if self.pending.startswith('\x1b['):
                match = re.match(r'\x1b\[([0-9;?<>]*)([ -/]*)([@-~])', self.pending)
                if not match:
                    break
                args, _, command = match.groups()
                self.pending = self.pending[match.end():]
                numbers = [int(v) if v else 0 for v in args.lstrip('?<>').split(';')]
                n = numbers[0] or 1
                if command in ('H', 'f'):
                    self.y = min(self.height - 1, n - 1)
                    self.x = min(self.width - 1, (numbers[1] or 1) - 1 if len(numbers) > 1 else 0)
                elif command == 'J' and numbers[0] in (2, 3):
                    self.cells = [[' '] * self.width for _ in range(self.height)]
                    self.wrapped = False
                elif args == '?2026' and command in ('h', 'l'):
                    self.complete = command == 'l'
                elif command == 'K':
                    start = 0 if numbers[0] in (1, 2) else self.x
                    end = self.x + 1 if numbers[0] == 1 else self.width
                    self.cells[self.y][start:end] = [' '] * (end - start)
                elif command == 'A':
                    self.y = max(0, self.y - n)
                elif command == 'B':
                    self.y = min(self.height - 1, self.y + n)
                elif command == 'C':
                    self.x = min(self.width - 1, self.x + n)
                elif command == 'D':
                    self.x = max(0, self.x - n)
                continue
            if self.pending[0] == '\x1b':
                if len(self.pending) == 1:
                    break
                self.pending = self.pending[2:]
                continue
            ch, self.pending = self.pending[0], self.pending[1:]
            if ch == '\r':
                self.x = 0
            elif ch == '\n':
                self.y = min(self.height - 1, self.y + 1)
            elif ch == '\b':
                self.x = max(0, self.x - 1)
            elif ch.isprintable():
                width = 0 if unicodedata.combining(ch) else (2 if unicodedata.east_asian_width(ch) in ('F', 'W') else 1)
                if self.x + width > self.width:
                    self.wrapped = True
                    self.x = 0
                    self.y = min(self.height - 1, self.y + 1)
                if width == 0 and self.x:
                    self.cells[self.y][self.x - 1] += ch
                elif self.x < self.width:
                    self.cells[self.y][self.x] = ch
                    self.x += width
        assert len(self.pending) < 8192

    def text(self):
        return '\n'.join(''.join(row).rstrip() for row in self.cells)


class API(http.server.BaseHTTPRequestHandler):
    failed = False
    delay = 0
    requests = 0

    def log_message(self, *_):
        pass

    def do_GET(self):
        type(self).requests += 1
        if self.delay:
            time.sleep(self.delay)
        status = 503 if self.failed else 200
        routes = [{'@id': f'sample-{i:02}', 'match': [{'host': [f'sample{i:02}.example.test']}],
                   'handle': [{'handler': 'reverse_proxy', 'upstreams': [{'dial': '127.0.0.1:3000'}]}]}
                  for i in range(30)]
        body = json.dumps({'apps': {'http': {'servers': {'edge': {'listen': [':8080'], 'routes': routes}}}}}).encode()
        try:
            self.send_response(status)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass


class Server(http.server.ThreadingHTTPServer):
    # Numeric binding avoids the macOS framework interpreter's reverse-DNS startup.
    server_bind = socketserver.TCPServer.server_bind
    daemon_threads = True


class TraefikAPI(http.server.BaseHTTPRequestHandler):
    responses = json.loads((Path(__file__).resolve().parents[1]/'fixtures/traefik/snapshot.json').read_text())['responses']

    def log_message(self, *_):
        pass

    def do_GET(self):
        value = self.responses.get(self.path.partition('?')[0])
        body = json.dumps(value).encode()
        self.send_response(200 if value is not None else 404)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def exercise(binary, captures=None):
    with tempfile.TemporaryDirectory(prefix='steward-tui-', dir=os.environ.get('TMPDIR')) as folder:
        server = Server(('127.0.0.1', 0), API)
        traefik = Server(('127.0.0.1', 0), TraefikAPI)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        threading.Thread(target=traefik.serve_forever, daemon=True).start()
        endpoint = f'http://127.0.0.1:{server.server_address[1]}'
        traefik_endpoint = f'http://127.0.0.1:{traefik.server_address[1]}'
        root = Path(folder)
        (root/'config/caddy').mkdir(parents=True)
        (root/'config/traefik').mkdir()
        (root/'home').mkdir()
        (root/'config/caddy/caddy.json').write_text(json.dumps({'admin': {'listen': f'127.0.0.1:{server.server_address[1]}'}}))
        (root/'config/traefik/traefik.json').write_text(json.dumps({'api': {'insecure': True}, 'entryPoints': {'traefik': {'address': f':{traefik.server_address[1]}'}}}))
        intent = root/'intent.json'
        intent.write_text(json.dumps({'schema_version': '1', 'scope': 'user', 'routes': [], 'services': []}))
        saved = root/'reviewed.json'
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 120, 0, 0))
        original = termios.tcgetattr(slave)
        env = dict(os.environ, HOME=str(root/'home'), XDG_CONFIG_HOME=str(root/'config'))
        process = subprocess.Popen([str(binary), '--state-dir', folder + '/state', 'tui'], env=env, stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
        screen = Screen(120, 30)
        budget = 0

        def read_until(*texts, timeout=20):
            nonlocal budget
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if select.select([master], [], [], .05)[0]:
                    data = os.read(master, 65536)
                    budget += len(data)
                    assert budget < 8 * 1024 * 1024, 'PTY output exceeded 8 MiB'
                    screen.feed(data)
                visible = screen.text()
                if screen.complete and not screen.wrapped and all(text in visible for text in texts):
                    return visible
                if process.poll() is not None:
                    break
            raise AssertionError((texts, process.poll(), screen.text()))

        def send(data):
            os.write(master, data)

        def paste(text):
            send(b'\x1b[200~' + text.encode() + b'\x1b[201~')

        def resize(width, height):
            screen.resize(width, height)
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', height, width, 0, 0))
            os.kill(process.pid, signal.SIGWINCH)

        def capture(name):
            if captures:
                captures.mkdir(parents=True, exist_ok=True)
                (captures/(name+'.txt')).write_text(screen.text()+'\n')

        try:
            read_until('[1] Routers', '[2] Routes', '[3] Services', '[4] Operations', '[5] Inspector', '[6] Activity')
            read_until('Discovery complete', endpoint)
            send(b'/')
            paste(traefik_endpoint)
            read_until('engine: traefik', 'route count: 2')
            send(b'\r')
            send(b'\r')
            read_until('photos.example.test', 'old.example.test')
            send(b'1')
            send(b'/')
            send(b'\x15')
            paste(endpoint)
            read_until('route count: 30', endpoint)
            send(b'\r')
            assert not (root/'state/state.json').exists(), 'Automatic discovery must not save connections or grant ownership'
            capture('workspace-discovered')
            send(b'\r')
            read_until('sample00', '[2] Routes  30')
            send(b'\x1b[F')
            read_until('sample29')
            send(b'/')
            paste('sample03')
            read_until('[2] Routes  1 /sample03')
            send(b'\r3')
            read_until('No services')
            send(b'2')
            read_until('[2] Routes  1 /sample03')
            send(b'\x1b')
            read_until('[2] Routes  30')
            send(b'1')
            read_until('route count: 30')
            send(b'c')
            read_until('Connect a proxy', 'Connection ID', 'Ctrl-S')
            capture('connect-dialog')
            send(b'\x15')
            send(b'\x13')
            read_until('Connection ID is required')
            paste('edge')
            send(b'\x13')
            read_until('[Result]', 'connected', 'edge')
            assert json.loads((root/'state/state.json').read_text())['connections'][0]['binding']['endpoint'] == endpoint
            send(b'1')
            read_until('edge live')
            send(b'\r')
            read_until('edge', '[2] Routes  30')
            send(b'\x1b[F')
            read_until('sample29')
            before = API.requests
            send(b'\r')
            read_until('Resource details: sample-29:0 · observed', 'sample29.example.test')
            assert API.requests > before, 'Enter must fetch an inspection, not expand a cached row'
            capture('route-inspector')
            send(b'z')
            read_until('[5] Inspector', 'Resource details:')
            send(b'\x1b[F')
            read_until('tls configured')
            send(b'z')
            read_until('[2] Routes', '[6] Activity')
            send(b'\x1b')
            read_until('Enter inspect')
            send(b'a')
            menu = read_until('Action palette', 'Inspect resource')
            assert 'Disable route' not in menu and 'Remove owned route' not in menu
            send(b'\x1b')
            read_until('Enter inspect')
            send(b'p')
            paste('Create saved plan')
            read_until('Action palette', 'Create saved plan')
            send(b'\r')
            read_until('Manifest file', 'Save plan to')
            paste(str(intent))
            send(b'\x0f')
            read_until('Select a file or directory', 'intent.json')
            send(b'\x1b')
            read_until('Create saved plan', 'Manifest file')
            send(b'\t')
            paste(str(saved))
            send(b'\x13')
            read_until('[Result]', 'planned', 'applicable: true')
            assert json.loads(saved.read_text())['kind'] == 'steward.route-plan'
            capture('plan-review')
            send(b'A')
            read_until('Apply reviewed plan', 'reviewed.json')
            send(b'\x13')
            read_until('[Result]', 'applied', 'pruned: false')
            send(b'1')
            read_until('edge live')
            send(b'a')
            paste('Remove connection')
            read_until('Remove connection')
            send(b'\r')
            read_until('Confirm selected action', 'Esc cancel')
            send(b'\x1b')
            read_until('edge live')
            assert len(json.loads((root/'state/state.json').read_text())['connections']) == 1

            API.failed = True
            send(b'r')
            read_until('edge unavailable')
            send(b'i')
            read_until('Inventory diagnostics', 'connection.status', 'HTTP 503')
            send(b'\x1b')
            read_until('unavailable')
            API.failed = False
            send(b'r')
            read_until('edge live')
            send(b':routes remove\r')
            read_until('Command help / error', 'Usage:')
            send(b'\x1b')
            read_until('edge live')
            send(b'?')
            read_until('Keyboard help', 'WORKSPACE')
            send(b'\x1b[F')
            read_until('External routes require explicit adoption')
            send(b'\x1b')
            read_until('edge live')
            resize(80, 24)
            read_until('[1] Routers', '[2] Routes', '[5] Inspector', '[6] Activity', 'q quit')
            capture('workspace-80')
            resize(48, 14)
            read_until('[1] Routers', '[5] Inspector', 'q quit')
            resize(20, 5)
            read_until('Terminal too small', 'q quit')
            resize(80, 24)
            read_until('[1] Routers', 'q quit')
            API.delay = 4
            send(b'1')
            send(b'r')
            read_until('Loading Routers')
            send(b'c')
            read_until('Connect a proxy')
            paste('not-submitted')
            read_until('not-submitted')
            resize(20, 5)
            read_until('Terminal too small', 'q quit')
            started = time.monotonic()
            send(b'q')
            assert process.wait(timeout=2) == 0, 'Read-only refresh must not trap the user in a frozen UI'
            assert time.monotonic() - started < 2
            assert termios.tcgetattr(slave) == original, 'Terminal settings were not fully restored'
            print('TUI workspace PTY passed: automatic custom-port detection without registration, six persistent panels, router/route navigation, retained filters, modal connection, live inspection, zoom, searchable workflows, file picker, saved plan/apply, cancelable removal, diagnostics, 120/80/48-column layouts, responsive quit and terminal restoration.')
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            os.close(master)
            os.close(slave)
            server.shutdown()
            server.server_close()
            traefik.shutdown()
            traefik.server_close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--capture-dir', type=Path)
    args = parser.parse_args()
    exercise(args.binary.resolve(), args.capture_dir)
