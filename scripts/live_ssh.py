"""Exercise SSH delegation against a temporary loopback-only key-authenticated sshd."""
import argparse
import json
import os
from pathlib import Path
import pwd
import shutil
import subprocess
import sys
import tempfile
from live_route_smoke import port,request,wait

def exercise(binary,caddy):
    # sshd StrictModes rejects world-writable /tmp ancestors even with a private
    # fixture directory. Keep these test keys under the existing Cargo target.
    with tempfile.TemporaryDirectory(prefix='steward-ssh-',dir=Path(__file__).resolve().parents[1]/'target') as folder:
        root=Path(folder);tools=root/'tools';tools.mkdir();ssh_port,admin,web,backend=port(),port(),port(),port()
        for name in ('host','identity'):subprocess.run(['ssh-keygen','-q','-t','ed25519','-N','','-f',str(root/name)],check=True,timeout=10)
        (root/'authorized_keys').write_bytes((root/'identity.pub').read_bytes())
        user=pwd.getpwuid(os.getuid()).pw_name
        config=root/'sshd_config';config.write_text(f'Port {ssh_port}\nListenAddress 127.0.0.1\nHostKey {root}/host\nPidFile {root}/sshd.pid\nAuthorizedKeysFile {root}/authorized_keys\nPasswordAuthentication no\nKbdInteractiveAuthentication no\nUsePAM no\nAllowUsers {user}\nStrictModes yes\n')
        key=(root/'host.pub').read_text().split();(root/'known_hosts').write_text(f'[127.0.0.1]:{ssh_port} {key[0]} {key[1]}\n')
        client=root/'ssh_config';client.write_text(f'Host acceptance\n HostName 127.0.0.1\n Port {ssh_port}\n User {user}\n IdentityFile {root}/identity\n IdentitiesOnly yes\n UserKnownHostsFile {root}/known_hosts\n StrictHostKeyChecking yes\n')
        wrapper=tools/'ssh';wrapper.write_text(f'#!{sys.executable}\nimport os,sys\nos.execv("/usr/bin/ssh",["ssh","-F",{str(client)!r},*sys.argv[1:]])\n');wrapper.chmod(0o755)
        native={'admin':{'listen':f'127.0.0.1:{admin}'},'apps':{'http':{'servers':{'web':{'listen':[f'127.0.0.1:{web}'],'automatic_https':{'disable':True},'routes':[]},'backend':{'listen':[f'127.0.0.1:{backend}'],'automatic_https':{'disable':True},'routes':[{'handle':[{'handler':'static_response','body':'ssh backend'}]}]}}}}}
        source=root/'proxy.json';source.write_text(json.dumps(native))
        env=dict(os.environ,PATH=str(tools)+os.pathsep+os.environ['PATH'])
        sshd=None;proxy=None
        log=(root/'sshd.log').open('wb')
        try:
            sshd=subprocess.Popen([shutil.which('sshd') or '/usr/sbin/sshd','-D','-e','-f',str(config)],stdout=log,stderr=log)
            proxy=subprocess.Popen([str(caddy),'run','--config',str(source)],env={**env,'XDG_CONFIG_HOME':str(root/'config'),'XDG_DATA_HOME':str(root/'data')},stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            wait(lambda:request(f'http://127.0.0.1:{admin}/config/'))
            def ssh_ready():
                if sshd.poll() is not None:
                    log.flush()
                    with (root/'sshd.log').open('rb') as stream:raise AssertionError(stream.read(8192).decode())
                result=subprocess.run([str(wrapper),'-T','acceptance','true'],capture_output=True,timeout=5)
                if result.returncode:
                    raise OSError(result.stderr.decode()[:2048])
                return True
            try:wait(ssh_ready)
            except AssertionError:
                log.flush()
                with (root/'sshd.log').open('rb') as stream:print(stream.read(8192).decode(),file=sys.stderr)
                raise
            prefix=[str(binary),'--ssh','acceptance','--remote-bin',str(binary),'--state-dir',str(root/'remote state')]
            def cli(*args):
                result=subprocess.run(prefix+list(args)+['--json'],env=env,capture_output=True,timeout=90)
                assert result.returncode==0,(args,result.stdout.decode(),result.stderr.decode())
                return json.loads(result.stdout)
            cli('proxies','add','edge','--engine','caddy','--endpoint',f'http://127.0.0.1:{admin}','--config-file',str(source),'--listener','web','--caddy-bin',str(caddy))
            cli('routes','add','app','--proxy','edge','--host','ssh.localhost','--upstream',f'http://127.0.0.1:{backend}','--tls','http')
            assert request(f'http://127.0.0.1:{web}/','ssh.localhost')[0]==b'ssh backend'
            cli('routes','remove','app','--proxy','edge')
            print('SSH target: host-key verification, quoted paths with spaces, live route apply and removal passed.')
        finally:
            for child in (proxy,sshd):
                if child:
                    child.terminate()
                    try:child.wait(timeout=8)
                    except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
            log.close()

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--caddy',type=Path,required=True)
    args=parser.parse_args();exercise(args.binary.resolve(),args.caddy.resolve())
