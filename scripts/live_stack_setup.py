"""Real proxy installer, supervisor, service-route composition and upgrade acceptance."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from live_route_smoke import port,request,wait

def exercise(binary):
    with tempfile.TemporaryDirectory(prefix='steward-stack-',dir=os.environ.get('TMPDIR')) as folder:
        root=Path(folder);state=root/'state';admin,web,backend=port(),port(),port()
        prefix=[str(binary),'--state-dir',str(state)]
        def cli(*args):
            result=subprocess.run(prefix+list(args)+['--json'],capture_output=True,timeout=180)
            assert result.returncode==0,(args,result.stdout.decode(),result.stderr.decode())
            return json.loads(result.stdout)
        try:
            cli('proxies','install','edge','--engine','caddy','--version','2.11.7','--listen',f'127.0.0.1:{web}','--admin',f'127.0.0.1:{admin}')
            stack=root/'stack.json'
            spec={'id':'webapp','scope':'user','executable':sys.executable,'args':['-m','http.server',str(backend),'--bind','127.0.0.1'],'working_directory':str(root),'environment':{},'enabled':False,'restart':False,'ready_url':f'http://127.0.0.1:{backend}/'}
            stack.write_text(json.dumps({'schema_version':'1','id':'demo','scope':'user','services':[spec],'routes':[{'spec':{'id':'app','proxy':'edge','hostname':'app.localhost','upstream':{'url':f'http://127.0.0.1:{backend}'},'enabled':True,'tls':{'mode':'http'}},'depends_on':['webapp']}]}))
            cli('up','--file',str(stack));assert request(f'http://127.0.0.1:{web}/','app.localhost')[0]
            before=(state/'proxies/edge/proxy.json').read_bytes()
            cli('proxies','upgrade','edge','--version','2.11.7')
            assert (state/'proxies/edge/proxy.json').read_bytes()==before
            assert request(f'http://127.0.0.1:{web}/','app.localhost')[0]
            exported=root/'export.json';cli('export','--out',str(exported));export=json.loads(exported.read_text());assert export['kind']=='steward.inventory' and export['complete'] is True
            assert any(r['kind']=='service' and r['public_url'] is None for r in export['resources'])
            assert any(r['kind']=='route' and r['public_url']==f'http://app.localhost:{web}' for r in export['resources'])
            cli('down','demo');assert cli('services','inspect','webapp')['data']['running'] is True
            cli('services','stop','proxy-edge')
            assert cli('proxies','upgrade','edge','--version','2.11.7')['data']['running'] is False
            assert cli('services','inspect','proxy-edge')['data']['running'] is False
            cli('services','start','proxy-edge')
            cli('up','--file',str(stack));assert request(f'http://127.0.0.1:{web}/','app.localhost')[0]
            cli('down','demo','--remove-services');assert not any(s['id']=='webapp' for s in cli('services','list')['data'])
            interrupted=root/'interrupted.json'
            slow={**spec,'id':'interrupted','args':['-m','http.server',str(port()),'--bind','127.0.0.1'],'ready_url':'http://127.0.0.1:1/'}
            interrupted.write_text(json.dumps({'schema_version':'1','id':'interrupted','scope':'user','services':[slow],'routes':[]}))
            child=subprocess.Popen(prefix+['up','--file',str(interrupted),'--json'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            try:
                def native_started():
                    receipt=json.loads((state/'stacks/interrupted.json').read_text())
                    pending=receipt.get('pending_service')
                    if not pending:return False
                    operation=json.loads((state/'service-operation.json').read_text())
                    if operation['id']!=pending[1] or operation['phase']!='publishing':return False
                    result=subprocess.run(prefix+['services','inspect-native',operation['record']['label'],'--json'],capture_output=True,timeout=10)
                    if result.returncode:return False
                    fields=json.loads(result.stdout)['data']['native']
                    return fields.get('ActiveState')=='active' or fields.get('state')=='running'
                wait(native_started)
                child.terminate();child.wait(timeout=10)
            finally:
                if child.poll() is None:child.kill();child.wait(timeout=5)
            cli('services','recover');cli('down','interrupted','--remove-services')
            assert not any(s['id']=='interrupted' for s in cli('services','list')['data'])
            cli('proxies','uninstall','edge');assert (state/'proxies/edge/proxy.json').exists()
            assert not (state/'proxies/edge/caddy').exists()
            print('Proxy install/upgrade/uninstall, native startup/readiness, service-first up, route-first down and redacted export passed.')
        finally:
            for args in [('services','recover'),('down','interrupted','--remove-services'),('services','remove','interrupted'),('down','demo','--remove-services'),('routes','remove','app','--proxy','edge'),('services','remove','webapp'),('proxies','recover','edge'),('proxies','uninstall','edge'),('services','remove','proxy-edge')]:
                subprocess.run(prefix+list(args)+['--json'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=90)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,required=True)
    exercise(parser.parse_args().binary.resolve())
