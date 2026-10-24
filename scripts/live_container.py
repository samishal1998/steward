"""Run the installed CLI inside a disposable container and verify container-local writes."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid
from live_route_smoke import wait

IMAGE='ubuntu@sha256:68ee22004a9109b44973313b02ff7b1de250f1de0f5ecb628d73f579aaff33ac'

def exercise(binary,caddy):
    with tempfile.TemporaryDirectory(prefix='steward-container-',dir=os.environ.get('TMPDIR')) as folder:
        root=Path(folder);name='steward-target-'+uuid.uuid4().hex[:12]
        shutil.copy2(binary,root/'steward');shutil.copy2(caddy,root/'caddy')
        (root/'home').mkdir()
        config={'admin':{'listen':'127.0.0.1:2019'},'apps':{'http':{'servers':{'web':{'listen':['127.0.0.1:8088'],'automatic_https':{'disable':True},'routes':[]},'backend':{'listen':['127.0.0.1:5678'],'automatic_https':{'disable':True},'routes':[{'handle':[{'handler':'static_response','body':'inside container'}]}]}}}}}
        (root/'proxy.json').write_text(json.dumps(config))
        prefix=[str(binary),'--container',name,'--remote-bin','/test/steward','--state-dir','/test/state']
        def cli(*args):
            result=subprocess.run(prefix+list(args)+['--json'],capture_output=True,timeout=90)
            assert result.returncode==0,(args,result.stdout.decode(),result.stderr.decode())
            return json.loads(result.stdout)
        try:
            subprocess.run(['docker','run','-d','--name',name,'--user',f'{os.getuid()}:{os.getgid()}','--env','HOME=/test/home','--env','XDG_DATA_HOME=/test/data','--env','XDG_CONFIG_HOME=/test/config','--mount',f'type=bind,src={root},dst=/test','--entrypoint','/test/caddy',IMAGE,'run','--config','/test/proxy.json'],check=True,stdout=subprocess.DEVNULL,timeout=60)
            wait(lambda:subprocess.run(['docker','exec',name,'/test/steward','--version'],capture_output=True,timeout=5).returncode==0)
            cli('proxies','add','local','--engine','caddy','--endpoint','http://127.0.0.1:2019','--config-file','/test/proxy.json','--listener','web','--caddy-bin','/test/caddy')
            cli('routes','add','app','--proxy','local','--host','app.localhost','--upstream','http://127.0.0.1:5678','--tls','http')
            assert any(r['ownership']=='owned' for r in cli('routes','list')['data'])
            cli('routes','remove','app','--proxy','local')
            assert (root/'state/state.json').exists()
            print('Container target: target-local source/state, live routing verification and removal passed.')
        finally:subprocess.run(['docker','rm','-f',name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=30)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--caddy',type=Path,required=True)
    args=parser.parse_args();exercise(args.binary.resolve(),args.caddy.resolve())
