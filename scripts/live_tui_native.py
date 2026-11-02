"""Drive real route/service/rollback workflows in a task-owned isolated tmux server."""
import argparse
import contextlib
import json
import os
import re
import shutil
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from live_route_smoke import port, wait


@contextlib.contextmanager
def native_workspace():
    folder = tempfile.mkdtemp(prefix='steward-native-ui-', dir=os.environ.get('TMPDIR'))
    try:
        yield folder
    except BaseException:
        print('Preserved failed native TUI workspace:', folder, file=sys.stderr)
        raise
    else:
        shutil.rmtree(folder)


def exercise(binary, caddy, captures=None):
    with native_workspace() as folder:
        root = Path(folder)
        state = root/'state'
        socket = root/'tmux.sock'
        assert len(str(socket)) < 100
        admin, frontend, backend, stack_port = port(), port(), port(), port()
        endpoint = f'http://127.0.0.1:{admin}'
        source = root/'caddy.json'
        source.write_text(json.dumps({'admin': {'listen': f'127.0.0.1:{admin}'}, 'apps': {'http': {'servers': {'web': {'listen': [f'127.0.0.1:{frontend}'], 'automatic_https': {'disable': True}, 'routes': []}}}}}))
        (root/'index.html').write_text('steward native workspace\n')
        proxy = subprocess.Popen([str(caddy), 'run', '--config', str(source)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                 env=dict(os.environ, XDG_DATA_HOME=str(root/'proxy-data'), XDG_CONFIG_HOME=str(root/'proxy-config')))
        prefix = [str(binary), '--state-dir', str(state), '--json']
        native = None
        started = False

        def cli(*args, required=True):
            result = subprocess.run(prefix+list(args), capture_output=True, timeout=70)
            if required:
                assert result.returncode == 0, (args, result.stdout.decode(), result.stderr.decode())
            return result

        def tmux(*args, required=True):
            result = subprocess.run(['tmux', '-f', '/dev/null', '-S', str(socket), *args], capture_output=True, timeout=10)
            if required:
                assert result.returncode == 0, result.stderr.decode()
            return result.stdout.decode(errors='replace')

        def key(*keys):
            tmux('send-keys', '-t', 'ui', *keys)

        def text(value):
            tmux('send-keys', '-t', 'ui', '-l', '--', value)

        def screen(*parts):
            deadline = time.monotonic()+40
            while time.monotonic() < deadline:
                output = tmux('capture-pane', '-p', '-t', 'ui')
                if all(part in output for part in parts):
                    return output
                time.sleep(.1)
            raise AssertionError((parts, output))

        def action(name, confirm=False):
            key('a')
            screen('Action palette')
            text(name)
            screen('/ '+name)
            key('Enter')
            if confirm:
                screen('Confirm selected action', name)
                key('Enter')
            status = 'applied' if 'Roll back' not in name else 'rolled_back'
            screen('[Result]', f'{name} · {status}')

        def http(url, host=None):
            request = urllib.request.Request(url, headers={} if host is None else {'Host': host})
            try:
                with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request, timeout=2) as response:
                    return response.status, response.read(4096)
            except urllib.error.HTTPError as error:
                return error.code, b''

        try:
            wait(lambda: http(endpoint+'/config/')[0] == 200)
            server = Path(__file__).resolve().parents[1]/'examples/static-server.py'
            cli('services', 'add', 'workspace-app', '--executable', sys.executable, '--working-directory', str(root),
                '--ready-url', f'http://127.0.0.1:{backend}/', '--env', 'TOKEN=native-ui-private', '--', str(server), '--port', str(backend))
            native = Path(json.loads((state/'services.json').read_text())['services'][0]['path'])
            tmux('new-session', '-d', '-s', 'ui', '-x', '140', '-y', '40', str(binary), '--state-dir', str(state), 'tui')
            started = True
            screen('Discovery complete', '[1] Routers')
            key('/')
            text(endpoint)
            screen('runtime confirmed: true', endpoint)
            key('Enter')
            key('c')
            screen('Connect a proxy')
            key('C-u'); text('edge')
            key('Tab', 'Tab', 'Tab', 'Right', 'Tab'); text(str(source))
            key('Tab'); text('web')
            key('Tab', 'Tab', 'C-u'); text(str(caddy))
            key('C-s')
            screen('[Result]', 'connected')
            key('3')
            screen('workspace-app')
            action('Start service')
            assert http(f'http://127.0.0.1:{backend}/')[0] == 200
            key('L')
            screen('[Logs]', 'Service logs · observed')
            if captures:
                captures.mkdir(parents=True, exist_ok=True)
                (captures/'native-service-logs.txt').write_text(tmux('capture-pane', '-p', '-t', 'ui'))
            key('1'); screen('edge live'); key('Enter'); screen('[2] Routes')
            key('n')
            form = screen('Add a route')
            assert re.search(r'Proxy\s+\[ edge \]', form), 'Selected router must prefill the route form'
            text('app'); key('Tab')
            key('Tab'); text('app.localhost')
            key('Tab', 'C-u'); text(f'http://127.0.0.1:{backend}')
            key('C-s'); screen('[Result]', 'applied')
            assert http(f'http://127.0.0.1:{frontend}/', 'app.localhost') == (200, b'steward native workspace\n')
            key('2'); screen('app.localhost')
            action('Disable route')
            assert http(f'http://127.0.0.1:{frontend}/', 'app.localhost')[1] != b'steward native workspace\n'
            key('2'); screen('app.localhost')
            action('Enable route')
            assert http(f'http://127.0.0.1:{frontend}/', 'app.localhost')[0] == 200
            key('2'); screen('app.localhost')
            action('Remove owned route', confirm=True)
            assert http(f'http://127.0.0.1:{frontend}/', 'app.localhost')[1] != b'steward native workspace\n'
            operation = json.loads((state/'operation.json').read_text())['id']
            key('4', '/')
            text(operation); key('Enter'); screen(operation)
            action('Roll back checked operation', confirm=True)
            assert http(f'http://127.0.0.1:{frontend}/', 'app.localhost')[0] == 200
            if captures:
                (captures/'native-rollback.txt').write_text(tmux('capture-pane', '-p', '-t', 'ui'))
            route_spec = {'id': 'planned', 'proxy': 'edge', 'hostname': 'plan.localhost', 'upstream': {'url': f'http://127.0.0.1:{backend}'}, 'enabled': True, 'tls': {'mode': 'http'}}
            manifest = root/'intent.json'; plan = root/'reviewed.json'
            manifest.write_text(json.dumps({'schema_version': '1', 'scope': 'user', 'routes': [route_spec], 'services': []}))
            key('P'); screen('Create saved plan', 'Manifest file')
            text(str(manifest)); key('Tab'); text(str(plan)); key('C-s')
            screen('[Result]', 'REVIEW PLANNED ROUTE CHANGES', 'UPSERT planned')
            assert http(f'http://127.0.0.1:{frontend}/', 'plan.localhost')[1] != b'steward native workspace\n'
            key('A'); screen('Apply reviewed plan', 'reviewed.json'); key('C-s')
            screen('[Result]', 'Apply reviewed plan · applied')
            assert http(f'http://127.0.0.1:{frontend}/', 'plan.localhost')[0] == 200
            route_spec.update(id='stack-route', hostname='stack.localhost', upstream={'url': f'http://127.0.0.1:{stack_port}'})
            stack = root/'stack.json'
            stack.write_text(json.dumps({'schema_version': '1', 'id': 'workspace-stack', 'scope': 'user',
                                        'services': [{'id': 'stack-app', 'scope': 'user', 'executable': sys.executable, 'args': [str(server), '--port', str(stack_port)], 'working_directory': str(root), 'environment': {}, 'enabled': False, 'restart': False, 'ready_url': f'http://127.0.0.1:{stack_port}/', 'ready_timeout': 20}],
                                        'routes': [{'spec': route_spec, 'depends_on': ['stack-app']}]}))
            key('u'); screen('Bring stack up', 'Stack file'); text(str(stack)); key('C-s')
            screen('[Result]', 'Bring stack up · applied')
            assert http(f'http://127.0.0.1:{frontend}/', 'stack.localhost')[0] == 200
            key('D'); screen('Bring stack down', 'Stack ID'); text('workspace-stack'); key('Tab', 'Right', 'C-s')
            screen('[Result]', 'Bring stack down · applied')
            assert http(f'http://127.0.0.1:{frontend}/', 'stack.localhost')[1] != b'steward native workspace\n'
            assert [r['spec']['id'] for r in json.loads((state/'services.json').read_text())['services']] == ['workspace-app']
            key('3'); screen('workspace-app')
            action('Stop service')
            key('e')
            editor = screen('Edit native service', 'Ready timeout')
            assert 'native-ui-private' not in editor
            key(*(['Tab']*9), 'C-u'); text('47'); key('C-s')
            screen('[Result]', 'Edit native service · applied')
            spec = json.loads((state/'services.json').read_text())['services'][0]['spec']
            assert spec['ready_timeout'] == 47 and spec['environment']['TOKEN'] == 'native-ui-private'
            key('q')
            deadline = time.monotonic()+5
            while time.monotonic() < deadline and tmux('list-sessions', required=False):
                time.sleep(.1)
            assert not tmux('list-sessions', required=False), 'TUI did not exit normally'
            print('Native TUI/tmux passed: Caddy auto-detection, writable connection, native service start/logs/stop/edit with environment retention, route lifecycle/rollback, actual saved-plan apply, readiness-gated stack up/down and verified real traffic.')
        finally:
            try:
                if started:
                    tmux('kill-server', required=False)
                if (state/'operation.json').exists():
                    pending = json.loads((state/'operation.json').read_text())
                    if pending['phase'] not in ('verified', 'compensated'):
                        cli('operations', 'recover', pending['id'], required=False)
                cli('routes', 'remove', 'app', '--proxy', 'edge', required=False)
                cli('routes', 'remove', 'planned', '--proxy', 'edge', required=False)
                cli('down', 'workspace-stack', '--remove-services', required=False)
                cli('services', 'recover', required=False)
                cli('services', 'remove', 'stack-app', required=False)
                if (state/'services.json').exists():
                    assert all(r['spec']['id'] != 'stack-app' for r in json.loads((state/'services.json').read_text())['services']), 'Test stack service remains'
                if native is not None:
                    cli('services', 'remove', 'workspace-app')
                    assert not native.exists(), 'Test-created native service definition remains'
            finally:
                proxy.terminate()
                try:
                    proxy.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    proxy.kill(); proxy.wait(timeout=5)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--caddy', type=Path, required=True)
    parser.add_argument('--capture-dir', type=Path)
    args = parser.parse_args()
    exercise(args.binary.resolve(), args.caddy.resolve(), args.capture_dir)
