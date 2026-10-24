"""Exercise real local proxy writes in disposable processes, never host services."""
import argparse
import fcntl
import http.server
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request


class Backend(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        time.sleep(getattr(self.server, 'delay', 0))
        self.send_response(200)
        self.end_headers()
        try:
            self.wfile.write((self.server.label + ' ' + self.path).encode())
        except (BrokenPipeError, ConnectionResetError):
            pass

    def log_message(self, *_):
        pass


def port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]


def request(url, host=None, method='GET', data=None, headers=None):
    extra = dict(headers or {})
    if host:
        extra['Host'] = host
    if data is not None:
        extra['Content-Type'] = 'application/json'
        data = json.dumps(data).encode()
    req = urllib.request.Request(url, data=data, headers=extra, method=method)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(req, timeout=5) as response:
        return response.read(1024 * 1024), dict(response.headers)


def wait(check, seconds=10):
    deadline = time.monotonic() + seconds
    last = None
    while time.monotonic() < deadline:
        try:
            value = check()
            if value:
                return value
        except (OSError, urllib.error.URLError, json.JSONDecodeError) as exc:
            last = exc
        time.sleep(.05)
    raise AssertionError(f'Deadline exceeded: {last}')


def run_engine(engine, executable, binary):
    with tempfile.TemporaryDirectory(prefix='steward-live-', dir=os.environ.get('TMPDIR')) as directory:
        root = Path(directory)
        for name in ('home', 'data', 'config', 'dynamic'):
            (root / name).mkdir()
        env = dict(os.environ, HOME=str(root / 'home'), XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'))
        api_port, web_port = port(), port()
        api, web = f'http://127.0.0.1:{api_port}', f'http://127.0.0.1:{web_port}'
        servers, threads = [], []
        for label, delay in [('one', 0), ('two', 0), ('slow', 10)]:
            server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Backend)
            server.label, server.delay = label, delay
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            servers.append(server)
            threads.append(thread)
        backends = [f'http://127.0.0.1:{s.server_port}' for s in servers]
        source = root / ('caddy.json' if engine == 'caddy' else 'traefik.yaml')
        state = root / 'state'
        prefix = [str(binary), '--state-dir', str(state)]
        child = None
        try:
            if engine == 'caddy':
                external = {'match': [{'host': ['external.localhost']}], 'handle': [{'handler': 'static_response', 'body': 'external'}], 'terminal': True}
                initial = {'admin': {'listen': f'127.0.0.1:{api_port}'}, 'apps': {'http': {'servers': {'web': {'listen': [f'127.0.0.1:{web_port}'], 'automatic_https': {'disable': True}, 'routes': [external]}}}}}
                source.write_text(json.dumps(initial))
                native_args = [str(executable), 'run', '--config', str(source)]
                connect = ['--listener', 'web', '--caddy-bin', str(executable)]
                external_path = source
            else:
                initial = {'entryPoints': {'web': {'address': f'127.0.0.1:{web_port}'}, 'traefik': {'address': f'127.0.0.1:{api_port}'}}, 'api': {'insecure': True}, 'providers': {'file': {'directory': str(root / 'dynamic'), 'watch': True}}, 'log': {'level': 'ERROR'}}
                source.write_text(json.dumps(initial))
                external = {'http': {'routers': {'external': {'rule': 'Host(`external.localhost`)', 'entryPoints': ['web'], 'service': 'external'}}, 'services': {'external': {'loadBalancer': {'servers': [{'url': backends[0]}]}}}}}
                external_path = root / 'dynamic/external.yaml'
                external_path.write_text(json.dumps(external))
                native_args = [str(executable), '--configFile=' + str(source)]
                connect = ['--listener', 'web', '--directory', str(root / 'dynamic')]
            child = subprocess.Popen(native_args, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            wait(lambda: request(api + ('/config/' if engine == 'caddy' else '/api/http/routers')))
            original_external = external_path.read_text()

            def cli(*args, success=True):
                result = subprocess.run(prefix + list(args) + ['--json'], env=env, capture_output=True, timeout=45)
                assert (result.returncode == 0) == success, (args, result.returncode, result.stdout.decode(), result.stderr.decode())
                assert not result.stderr, result.stderr.decode()
                return json.loads(result.stdout)

            cli('proxies', 'add', 'local', '--engine', engine, '--endpoint', api, '--config-file', str(source), *connect)
            readonly = [str(binary), '--state-dir', str(root / 'readonly')]
            result = subprocess.run(readonly + ['proxies', 'add', 'inventory', '--engine', engine, '--endpoint', api, '--json'], env=env, capture_output=True, timeout=40)
            assert result.returncode == 0, result.stdout.decode()
            result = subprocess.run(readonly + ['routes', 'list', '--json'], env=env, capture_output=True, timeout=40)
            assert result.returncode == 0 and json.loads(result.stdout)['changed'] is False
            result = subprocess.run(readonly + ['routes', 'add', 'not-authorized', '--proxy', 'inventory', '--host', 'none.localhost', '--upstream', backends[0], '--tls', 'http', '--json'], env=env, capture_output=True, timeout=10)
            assert result.returncode == 2 and json.loads(result.stdout)['diagnostics'][0]['code'] == 'capability.read_only'
            result = subprocess.run(readonly + ['proxies', 'update', 'inventory', '--engine', engine, '--endpoint', api, '--json'], env=env, capture_output=True, timeout=40)
            assert result.returncode == 0, result.stdout.decode()
            result = subprocess.run(readonly + ['proxies', 'remove', 'inventory', '--json'], env=env, capture_output=True, timeout=10)
            assert result.returncode == 0
            for shell in ('bash', 'zsh', 'fish', 'elvish', 'powershell'):
                completion = subprocess.run(prefix + ['--config', '/never/read', 'completions', shell], env=env, capture_output=True, timeout=10)
                assert completion.returncode == 0 and b'upstream' in completion.stdout, (shell, completion.stderr.decode())
                if shell == 'bash':
                    syntax = subprocess.run(['bash', '-n'], input=completion.stdout, capture_output=True, timeout=10)
                    assert syntax.returncode == 0, syntax.stderr
            cli('routes', 'add', 'site', '--proxy', 'local', '--host', 'site.localhost', '--upstream', backends[0], '--tls', 'http')
            assert request(web + '/some/path?x=1', 'site.localhost')[0] == b'one /some/path?x=1'
            rows = cli('routes', 'list')['data']
            assert any(r['ownership'] == 'owned' for r in rows)
            assert cli('routes', 'inspect', 'site', '--proxy', 'local')['data']['ownership'] == 'owned'
            completion = subprocess.run([str(binary), '--', 'steward', '--state-dir', str(state), 'routes', 'inspect', 'si'], env={**env, 'STEWARD_COMPLETE':'bash', '_CLAP_COMPLETE_INDEX':'5'}, capture_output=True, timeout=10)
            assert completion.returncode == 0 and b'site' in completion.stdout, completion
            blocked = cli('proxies', 'remove', 'local', success=False)
            assert blocked['diagnostics'][0]['code'] == 'binding.in_use'
            update = cli('routes', 'update', 'site', '--proxy', 'local', '--upstream', backends[1])
            assert request(web + '/', 'site.localhost')[0] == b'two /'
            operation_id = update['data']['operation']
            assert cli('operations','inspect',operation_id)['data']['phase'] == 'verified'
            rollback = cli('operations','rollback',operation_id)
            assert rollback['data']['reverses'] == operation_id
            assert request(web + '/', 'site.localhost')[0] == b'one /'
            cli('routes', 'update', 'site', '--proxy', 'local', '--upstream', backends[1])
            assert len(cli('operations','list')['data']) >= 4
            manifest = root / 'intent.json'
            manifest.write_text(json.dumps({'schema_version':'1','scope':'user','routes':[{'id':'planned','proxy':'local','hostname':'planned.localhost','upstream':{'url':backends[0]},'enabled':True,'tls':{'mode':'http'}}]}))
            planned = root / 'reviewed.json'
            assert cli('plan','--file',str(manifest),'--out',str(planned))['data']['applicable'] is True
            plan_source=source if engine=='caddy' else next((root/'dynamic').glob('steward-*.yaml'))
            plan_before=plan_source.read_text();plan_source.write_text(plan_before+'\n')
            assert cli('apply','--plan',str(planned),'--yes',success=False)['diagnostics'][0]['code']=='plan.stale'
            plan_source.write_text(plan_before)
            cli('apply','--plan',str(planned),'--yes')
            assert request(web + '/', 'planned.localhost')[0] == b'one /'
            assert cli('apply','--plan',str(planned),'--yes',success=False)['diagnostics'][0]['code'] == 'plan.stale'
            cli('routes','remove','planned','--proxy','local')
            overlap = cli('routes', 'add', 'conflict', '--proxy', 'local', '--host', 'external.localhost', '--upstream', backends[0], '--tls', 'http', success=False)
            assert overlap['changed'] is False and overlap['diagnostics'][0]['code'] == 'route.external_overlap'

            managed_file = source if engine == 'caddy' else next((root / 'dynamic').glob('steward-*.yaml'))
            before_drift = managed_file.read_text()
            drift = json.loads(before_drift)
            if engine == 'caddy':
                drift['apps']['http']['servers']['web']['routes'][-1]['handle'][0]['upstreams'][0]['dial'] = '127.0.0.1:1'
            else:
                name = next(iter(drift['http']['services']))
                drift['http']['services'][name]['loadBalancer']['servers'][0]['url'] = 'http://127.0.0.1:1'
            drift_text = json.dumps(drift)
            managed_file.write_text(drift_text)
            latest=cli('operations','list')['data'][0]['id']
            assert cli('operations','rollback',latest,success=False)['diagnostics'][0]['code']=='operation.drift'
            blocked = cli('routes', 'disable', 'site', '--proxy', 'local', success=False)
            assert blocked['changed'] is False and blocked['diagnostics'][0]['code'] == 'ownership.drift'
            assert managed_file.read_text() == drift_text
            managed_file.write_text(before_drift)
            wait(lambda: request(web + '/', 'site.localhost')[0] == b'two /')

            with (state / 'lock').open('r+') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                blocked = cli('routes', 'remove', 'site', '--proxy', 'local', success=False)
                assert blocked['diagnostics'][0]['code'] == 'operation.locked'
            input_scope = next(root.glob('.steward-*.lock')) if engine == 'caddy' else root / 'dynamic/.steward.lock'
            with input_scope.open('r+') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                blocked = cli('routes', 'remove', 'site', '--proxy', 'local', success=False)
                assert blocked['diagnostics'][0]['code'] == 'operation.locked'
            cli('routes', 'disable', 'site', '--proxy', 'local')
            try:
                assert request(web + '/', 'site.localhost')[0] != b'two /', 'Disabled route still forwards'
            except urllib.error.HTTPError as exc:
                assert exc.code == 404
            cli('routes', 'enable', 'site', '--proxy', 'local')
            assert request(web + '/', 'site.localhost')[0] == b'two /'
            cli('routes', 'remove', 'site', '--proxy', 'local')
            if engine == 'traefik':
                assert not managed_file.exists(), 'Final route removal must delete the owned aggregate'
                assert external_path.read_text() == original_external
            else:
                assert json.loads(source.read_text())['apps']['http']['servers']['web']['routes'] == [external]

            failed = cli('routes', 'add', 'bad', '--proxy', 'local', '--host', 'bad.localhost', '--upstream', 'http://127.0.0.1:1', '--tls', 'http', success=False)
            assert failed['changed'] is False and failed['diagnostics'][0]['code'] == 'operation.compensated', failed
            assert cli('operations', 'list')['data'][0]['phase'] == 'compensated'

            interrupted = subprocess.Popen(prefix + ['routes', 'add', 'interrupted', '--proxy', 'local', '--host', 'interrupted.localhost', '--upstream', backends[2], '--tls', 'http', '--json'], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                wait(lambda: json.loads((state / 'operation.json').read_text())['phase'] == 'activation_pending')
                interrupted.terminate()
                interrupted.wait(timeout=10)
            finally:
                if interrupted.poll() is None:
                    interrupted.kill()
                    interrupted.wait(timeout=5)
            operation = cli('operations', 'list')['data'][0]
            assert operation['phase'] not in ('verified', 'compensated')
            pending = cli('routes', 'add', 'blocked', '--proxy', 'local', '--host', 'blocked.localhost', '--upstream', backends[0], '--tls', 'http', success=False)
            assert pending['diagnostics'][0]['code'] == 'operation.pending'
            cli('operations', 'recover', operation['id'])
            assert cli('operations', 'list')['data'][0]['phase'] == 'compensated'
            assert not any(r['ownership'] == 'owned' for r in cli('routes', 'list')['data'])

            if engine == 'traefik':
                external_route = cli('routes','inspect','external@file','--proxy','local')['data']
                args_adopt = ('routes','adopt','adopted','--proxy','local','--native-id','external@file','--source',str(external_path),'--expected-fingerprint')
                assert cli(*args_adopt,'wrong',success=False)['diagnostics'][0]['code'] == 'adoption.stale'
                adopted = cli(*args_adopt,external_route['native_fingerprint'])
                assert cli('routes','inspect','adopted','--proxy','local')['data']['ownership'] == 'owned'
                cli('operations','rollback',adopted['data']['operation'])
                assert external_path.read_text() == original_external
                assert cli('routes','inspect','external@file','--proxy','local')['data']['ownership'] == 'observed_external'

            if engine == 'caddy':
                original, headers = request(api + '/config/')
                original_root = json.loads(original)
                adoptable = {'@id':'external-app','match':[{'host':['adopt.localhost']}], 'handle':[{'handler':'reverse_proxy','upstreams':[{'dial':f'127.0.0.1:{servers[0].server_port}'}]}], 'terminal':True}
                original_root['apps']['http']['servers']['web']['routes'].append(adoptable)
                source.write_text(json.dumps(original_root))
                request(api + '/config/',method='POST',data=original_root,headers={'If-Match':headers['Etag']})
                inspected = cli('routes','inspect','external-app:0','--proxy','local')['data']
                adopted = cli('routes','adopt','adopted','--proxy','local','--native-id','external-app:0','--expected-fingerprint',inspected['native_fingerprint'])
                assert cli('routes','inspect','adopted','--proxy','local')['data']['ownership']=='owned'
                cli('operations','rollback',adopted['data']['operation'])
                assert cli('routes','inspect','external-app:0','--proxy','local')['data']['ownership']=='observed_external'
                original, headers = request(api + '/config/')
                changed = json.loads(original)
                changed['apps']['http']['servers']['web']['routes'][0]['handle'][0]['body'] = 'manual'
                request(api + '/config/', method='POST', data=changed, headers={'If-Match': headers['Etag']})
                try:
                    request(api + '/config/', method='POST', data=json.loads(original), headers={'If-Match': headers['Etag']})
                    raise AssertionError('Stale ETag accepted')
                except urllib.error.HTTPError as exc:
                    assert exc.code == 412
                assert request(web + '/', 'external.localhost')[0] == b'manual'
            print(f'{engine}: live add/update/disable/enable/remove, path/query forwarding, external preservation, drift, lock, failed activation compensation, interrupted recovery, completions passed.')
        finally:
            if child:
                child.terminate()
                try:
                    child.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait(timeout=5)
            for server in servers:
                server.shutdown()
                server.server_close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--caddy', type=Path, required=True)
    parser.add_argument('--traefik', type=Path, required=True)
    args = parser.parse_args()
    for engine, native in [('caddy', args.caddy), ('traefik', args.traefik)]:
        run_engine(engine, native.resolve(), args.binary.resolve())
