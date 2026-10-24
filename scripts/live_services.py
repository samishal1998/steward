"""Exercise uniquely named user services on the real host supervisor, then remove them."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from live_route_smoke import port, request, wait

def exercise(binary):
    with tempfile.TemporaryDirectory(prefix='steward-services-',dir=os.environ.get('TMPDIR')) as folder:
        root=Path(folder);state=root/'state';listen=port()
        prefix=[str(binary),'--state-dir',str(state)]
        native=None
        def cli(*args,success=True):
            result=subprocess.run(prefix+list(args)+['--json'],capture_output=True,timeout=60)
            assert (result.returncode==0)==success,(args,result.stdout.decode(),result.stderr.decode())
            return json.loads(result.stdout)
        try:
            # Put --json before the explicit argv separator.
            result=subprocess.run(prefix+['--json','services','add','web','--executable',sys.executable,'--working-directory',str(root),'--ready-url',f'http://127.0.0.1:{listen}/','--start','--','-m','http.server',str(listen),'--bind','127.0.0.1'],capture_output=True,timeout=60)
            if result.returncode and sys.platform=='linux' and (state/'service-operation.json').exists():
                label=json.loads((state/'service-operation.json').read_text())['record']['label']
                subprocess.run(['journalctl','--user','--no-pager','--lines','20','--unit',label+'.service'],timeout=10)
            if result.returncode and sys.platform=='darwin':
                for path in state.glob('*.log'):
                    with path.open('rb') as stream:print(path.name,stream.read(8192).decode(errors='replace'),file=sys.stderr)
            assert result.returncode==0,(result.stdout.decode(),result.stderr.decode())
            registry=json.loads((state/'services.json').read_text());native=Path(registry['services'][0]['path'])
            assert cli('services','inspect','web')['data']['running'] is True
            cli('services','enable','web');assert cli('services','inspect','web')['data']['enabled'] is True
            cli('services','disable','web');assert cli('services','inspect','web')['data']['enabled'] is False
            cli('services','restart','web');assert request(f'http://127.0.0.1:{listen}/')[0]
            cli('services','logs','web','--lines','10')
            original=native.read_bytes();native.write_bytes(original+b'\n# external edit\n')
            assert cli('services','stop','web',success=False)['diagnostics'][0]['code']=='service.drift'
            native.write_bytes(original)
            stopped=cli('services','stop','web');assert cli('services','inspect','web')['data']['running'] is False
            cli('operations','rollback',stopped['data']['operation']);assert cli('services','inspect','web')['data']['running'] is True
            cli('services','stop','web')
            cli('services','start','web');assert cli('services','inspect','web')['data']['running'] is True
            cli('services','remove','web');assert not native.exists()
            assert cli('services','list')['data']==[]
            print('Native user services: add/start/readiness/inspect/enable/disable/restart/logs/drift/stop/remove passed.')
        finally:
            if state.exists():
                subprocess.run(prefix+['services','recover','--json'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=60)
                subprocess.run(prefix+['services','remove','web','--json'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=60)
            assert native is None or not native.exists(),'Test-owned native definition still exists; inspect before discarding test state'

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,required=True)
    exercise(parser.parse_args().binary.resolve())
