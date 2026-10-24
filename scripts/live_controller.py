"""Disposable pinned caddy-docker-proxy acceptance: timer, events, restart and preservation."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import uuid
from live_route_smoke import port,request,wait

IMAGE='lucaslorentz/caddy-docker-proxy@sha256:3e6cf4a6382da0cd14108a69929d7f8cc59e9a59cae4c9795071fa392ffa8036'

def docker(*args):return subprocess.check_output(['docker',*args],text=True,timeout=60).strip()

def exercise(binary,controller):
    with tempfile.TemporaryDirectory(prefix='steward-controller-',dir=os.environ.get('TMPDIR')) as folder:
        root=Path(folder);(root/'www').mkdir();(root/'www/index.html').write_text('controller backend')
        for name in ('home','data','config'):(root/name).mkdir()
        env=dict(os.environ,HOME=str(root/'home'),XDG_DATA_HOME=str(root/'data'),XDG_CONFIG_HOME=str(root/'config'))
        network='steward-test-'+uuid.uuid4().hex[:12];backend=network+'-backend';prefix_label='stewardtest'+uuid.uuid4().hex[:8]
        admin,web=port(),port();source=root/'Caddyfile';source.write_text(f'{{\n admin 127.0.0.1:{admin}\n auto_https off\n}}\n')
        child=None
        args=[str(controller),'docker-proxy','--caddyfile-path',str(source),'--label-prefix',prefix_label,'--polling-interval','1s','--ingress-networks',network,'--docker-apis-version','1.44']
        cli_prefix=[str(binary),'--state-dir',str(root/'state')]
        def cli(*args):
            output=subprocess.run(cli_prefix+list(args)+['--json'],env=env,capture_output=True,timeout=100)
            assert output.returncode==0,(args,output.stdout.decode(),output.stderr.decode())
            return json.loads(output.stdout)
        try:
            docker('network','create',network)
            docker('run','-d','--name',backend,'--network',network,'--label',f'{prefix_label}=http://external.localhost:{web}','--label',f'{prefix_label}.bind=127.0.0.1','--label',f'{prefix_label}.reverse_proxy={{{{upstreams 5678}}}}','-v',f'{root}/www:/srv:ro',IMAGE,'file-server','--listen',':5678','--root','/srv')
            address=json.loads(docker('inspect',backend))[0]['NetworkSettings']['Networks'][network]['IPAddress']
            child=subprocess.Popen(args,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            base=f'http://127.0.0.1:{web}'
            wait(lambda:request(base,'external.localhost')[0]==b'controller backend',seconds=30)
            cli('proxies','add','controller','--engine','caddy','--endpoint',f'http://127.0.0.1:{admin}','--config-file',str(source),'--listener',f'127.0.0.1:{web}','--caddyfile','--docker-controller','--caddy-bin',str(controller))
            cli('routes','add','app','--proxy','controller','--host','app.localhost','--upstream',f'http://{address}:5678','--tls','http')
            assert request(base,'app.localhost')[0]==b'controller backend'
            assert request(base,'external.localhost')[0]==b'controller backend'
            docker('restart',backend)
            wait(lambda:request(base,'app.localhost')[0]==b'controller backend')
            wait(lambda:request(base,'external.localhost')[0]==b'controller backend')
            child.terminate();child.wait(timeout=10)
            child=subprocess.Popen(args,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            wait(lambda:request(base,'app.localhost')[0]==b'controller backend',seconds=30)
            cli('routes','remove','app','--proxy','controller')
            assert request(base,'external.localhost')[0]==b'controller backend'
            assert 'steward ' not in source.read_text()
            print('caddy-docker-proxy 2.13.1: base-only timer activation, label preservation, later event, restart and removal passed.')
        finally:
            if child:
                child.terminate()
                try:child.wait(timeout=10)
                except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
            subprocess.run(['docker','rm','-f',backend],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=30)
            subprocess.run(['docker','network','rm',network],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=30)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--controller',type=Path,required=True)
    args=parser.parse_args();exercise(args.binary.resolve(),args.controller.resolve())
