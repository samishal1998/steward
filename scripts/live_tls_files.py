"""Real Caddyfile/import and verified internal TLS acceptance in a disposable host context."""
import argparse
import http.server
import json
import os
from pathlib import Path
import ssl
import subprocess
import tempfile
import threading
import urllib.request
from live_route_smoke import Backend, port, request, wait


def exercise(binary, caddy):
    with tempfile.TemporaryDirectory(prefix='steward-tls-', dir=os.environ.get('TMPDIR')) as folder:
        root = Path(folder)
        for name in ('home','data','config'):
            (root/name).mkdir()
        env = dict(os.environ, HOME=str(root/'home'), XDG_DATA_HOME=str(root/'data'), XDG_CONFIG_HOME=str(root/'config'))
        api_port, http_port, tls_port = port(), port(), port()
        source = root/'Caddyfile'
        external = root/'external.caddy'
        external.write_text(f'http://external.localhost:{http_port} {{\n bind 127.0.0.1\n respond "external"\n}}\n')
        source.write_text(f'{{\n admin 127.0.0.1:{api_port}\n http_port {http_port}\n https_port {tls_port}\n skip_install_trust\n auto_https disable_redirects\n}}\nimport external.caddy\n')
        backend = http.server.ThreadingHTTPServer(('127.0.0.1',0),Backend)
        backend.label = 'secure'
        thread = threading.Thread(target=backend.serve_forever,daemon=True); thread.start()
        child = subprocess.Popen([str(caddy),'run','--config',str(source),'--adapter','caddyfile'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        prefix = [str(binary),'--state-dir',str(root/'state')]

        def cli(*args, success=True):
            result = subprocess.run(prefix+list(args)+['--json'],env=env,capture_output=True,timeout=90)
            assert (result.returncode==0)==success,(args,result.stdout.decode(),result.stderr.decode())
            return json.loads(result.stdout)
        try:
            api=f'http://127.0.0.1:{api_port}'
            wait(lambda:request(api+'/config/'))
            cli('proxies','add','plain','--engine','caddy','--endpoint',api,'--config-file',str(source),'--listener',f'127.0.0.1:{http_port}','--caddyfile','--caddy-bin',str(caddy))
            cli('routes','add','app','--proxy','plain','--host','app.localhost','--upstream',f'http://127.0.0.1:{backend.server_port}','--tls','http')
            assert request(f'http://127.0.0.1:{http_port}/path?q=1','app.localhost')[0]==b'secure /path?q=1'
            assert request(f'http://127.0.0.1:{http_port}/','external.localhost')[0]==b'external'
            before = source.read_text()
            ca = root/'data/caddy/pki/authorities/local/root.crt'
            # Provision the test CA through Caddy's read-only PKI metadata endpoint,
            # with skip_install_trust already explicit in the isolated startup input.
            request(api+'/pki/ca/local')
            wait(lambda:ca.exists())
            cli('proxies','add','secure','--engine','caddy','--endpoint',api,'--config-file',str(source),'--listener',f'127.0.0.1:{tls_port}','--caddyfile','--caddy-bin',str(caddy),'--ca-file',str(ca))
            added=cli('routes','add','tlsapp','--proxy','secure','--host','secure.localhost','--upstream',f'http://127.0.0.1:{backend.server_port}','--tls','internal')
            assert added['data']['verification']['routing'] is True
            # curl --resolve preserves both SNI and Host while independently checking trust.
            checked=subprocess.run(['curl','--noproxy','*','--fail','--silent','--show-error','--cacert',str(ca),'--resolve',f'secure.localhost:{tls_port}:127.0.0.1',f'https://secure.localhost:{tls_port}/'],capture_output=True,timeout=10)
            assert checked.returncode==0 and checked.stdout==b'secure /',checked.stderr
            cli('operations','rollback',added['data']['operation'])
            assert source.read_text()==before
            assert external.read_text().endswith(' respond "external"\n}\n')
            cli('routes','remove','app','--proxy','plain')
            assert request(f'http://127.0.0.1:{http_port}/','external.localhost')[0]==b'external'
            print('Caddyfile/import HTTP and internal HTTPS: verified SNI/CA, external preservation and rollback passed.')
        finally:
            child.terminate()
            try: child.wait(timeout=8)
            except subprocess.TimeoutExpired: child.kill(); child.wait(timeout=5)
            backend.shutdown(); backend.server_close()


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--caddy',type=Path,required=True)
    args=parser.parse_args()
    exercise(args.binary.resolve(),args.caddy.resolve())
