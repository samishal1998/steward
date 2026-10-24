"""Temporary bounded native diagnostic for the feature branch's launchd readiness failure."""
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
import sys
import tempfile
import time
import urllib.request
import uuid
import socket
import fcntl

with tempfile.TemporaryDirectory(prefix='steward-launchd-probe-') as folder:
    root=Path(folder);label='steward-probe-'+uuid.uuid4().hex;domain=f'gui/{os.getuid()}';target=domain+'/'+label
    (root/'state').mkdir(mode=0o700)
    state_lock=(root/'state/lock').open('w+')
    fcntl.flock(state_lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
    definition=Path.home()/'Library/LaunchAgents'/f'{label}.plist';definition.parent.mkdir(parents=True,exist_ok=True)
    payload={'Label':label,'ProgramArguments':[sys.executable,'-m','http.server',str(port),'--bind','127.0.0.1'],'WorkingDirectory':str(root),'EnvironmentVariables':{},'RunAtLoad':False,'KeepAlive':False,'ThrottleInterval':1,'StandardOutPath':str(root/'state/stdout.log'),'StandardErrorPath':str(root/'state/stderr.log')}
    definition.write_bytes(plistlib.dumps(payload));definition.chmod(0o600)
    def ctl(*args):
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            process=subprocess.Popen(['launchctl',*args],stdin=subprocess.DEVNULL,stdout=stdout,stderr=stderr,process_group=0)
            process.wait(timeout=10);stdout.seek(0);stderr.seek(0)
            result=subprocess.CompletedProcess(args,process.returncode,stdout.read(1024*1024).decode(),stderr.read(1024*1024).decode())
        if result.returncode:print(args,result.returncode,result.stderr[:1000])
        return result
    try:
        print('Interpreter:',sys.executable,'Resolved:',Path(sys.executable).resolve())
        ctl('disable',target);ctl('enable',target);ctl('bootstrap',domain,str(definition));ctl('kickstart',target)
        opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
        ready=False
        for attempt in range(30):
            try:
                with opener.open(f'http://127.0.0.1:{port}/',timeout=1) as response:
                    if response.status==200:ready=True;break
            except Exception as e:
                if attempt in (0,5,15,29):
                    native=ctl('print',target).stdout
                    fields={}
                    for line in native.splitlines():
                        match=re.match(r'\s*(state|pid|last exit code) = (.+)',line)
                        if match:fields.setdefault(match[1],match[2])
                    print('Attempt',attempt,'HTTP',type(e).__name__,str(e)[:200],'native',fields)
                    if fields.get('pid','').isdigit():
                        subprocess.run(['ps','-p',fields['pid'],'-o','comm=,state=,wchan='],timeout=5)
                        subprocess.run(['lsof','-a','-p',fields['pid'],'-iTCP','-n','-P'],timeout=5)
            time.sleep(1)
        for name in ('stdout.log','stderr.log'):
            path=root/'state'/name
            if path.exists():
                with path.open('rb') as stream:print(name,stream.read(4096).decode(errors='replace'))
        if not ready:raise RuntimeError('Native launchd Python readiness failed independently of Steward')
        ctl('disable',target)
        with opener.open(f'http://127.0.0.1:{port}/',timeout=2) as response:assert response.status==200
        print('Native launchd readiness and persistent-disable ordering passed.')
    finally:
        ctl('bootout',target);definition.unlink(missing_ok=True)
