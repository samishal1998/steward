"""Verify internal HTTPS through native Caddy JSON and Traefik file-provider inputs."""
import argparse
import http.server
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
from live_route_smoke import Backend,port,request,wait

def create_test_certificates(root):
    ca=root/'ca.crt';key=root/'ca.key';leaf=root/'leaf.crt';leafkey=root/'leaf.key';csr=root/'leaf.csr'
    # Keep fixture CA extensions explicit and independent of platform
    # openssl.cnf defaults.
    ca_config=root/'ca.cnf'
    ca_config.write_text('[req]\nprompt=no\ndistinguished_name=dn\nx509_extensions=ca\n[dn]\nCN=Steward Test CA\n[ca]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\nsubjectKeyIdentifier=hash\n')
    leaf_config=root/'leaf.cnf'
    leaf_config.write_text('[req]\nprompt=no\ndistinguished_name=dn\n[dn]\nCN=secure.localhost\n')
    extensions=root/'leaf.ext'
    extensions.write_text('basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:secure.localhost\n')
    commands=[
        ['openssl','req','-x509','-newkey','rsa:2048','-sha256','-nodes','-keyout',str(key),'-out',str(ca),'-days','1','-config',str(ca_config)],
        ['openssl','req','-newkey','rsa:2048','-sha256','-nodes','-keyout',str(leafkey),'-out',str(csr),'-config',str(leaf_config)],
        ['openssl','x509','-req','-sha256','-in',str(csr),'-CA',str(ca),'-CAkey',str(key),'-CAcreateserial','-out',str(leaf),'-days','1','-extfile',str(extensions)],
        ['openssl','verify','-CAfile',str(ca),str(leaf)],
    ]
    for command in commands:
        result=subprocess.run(command,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,timeout=30)
        assert result.returncode==0,(command,result.stderr.decode(errors='replace')[-2048:])
    certificate=subprocess.check_output(['openssl','x509','-in',str(ca),'-noout','-text'],text=True,timeout=10)
    assert certificate.count('X509v3 Basic Constraints:')==1,certificate
    return ca,leaf,leafkey

def exercise(binary,engine,native):
    with tempfile.TemporaryDirectory(prefix='steward-native-tls-',dir=os.environ.get('TMPDIR')) as folder:
        root=Path(folder)
        for name in ('home','data','config','dynamic'):(root/name).mkdir()
        env=dict(os.environ,HOME=str(root/'home'),XDG_DATA_HOME=str(root/'data'),XDG_CONFIG_HOME=str(root/'config'))
        admin,tls_port,http_port=port(),port(),port();api=f'http://127.0.0.1:{admin}'
        source=root/'proxy.json'
        backend=http.server.ThreadingHTTPServer(('127.0.0.1',0),Backend);backend.label='native-tls';threading.Thread(target=backend.serve_forever,daemon=True).start()
        ca=root/'data/caddy/pki/authorities/local/root.crt'
        if engine=='caddy':
            config={'admin':{'listen':f'127.0.0.1:{admin}'},'apps':{'pki':{'certificate_authorities':{'local':{'install_trust':False}}},'http':{'http_port':http_port,'https_port':tls_port,'servers':{'secure':{'listen':[f'127.0.0.1:{tls_port}'],'tls_connection_policies':[{}],'automatic_https':{'disable_redirects':True},'routes':[]}}}}}
            args=[str(native),'run','--config',str(source)]
            flags=['--listener','secure','--caddy-bin',str(native),'--ca-file',str(ca)]
        else:
            ca,leaf,leafkey=create_test_certificates(root)
            config={'entryPoints':{'secure':{'address':f'127.0.0.1:{tls_port}'},'traefik':{'address':f'127.0.0.1:{admin}'}},'api':{'insecure':True},'providers':{'file':{'directory':str(root/'dynamic'),'watch':True}}}
            args=[str(native),'--configFile='+str(source)]
            flags=['--listener','secure','--directory',str(root/'dynamic'),'--ca-file',str(ca),'--certificate',str(leaf),'--private-key',str(leafkey)]
        source.write_text(json.dumps(config))
        child=subprocess.Popen(args,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        prefix=[str(binary),'--state-dir',str(root/'state')]
        def cli(*args):
            result=subprocess.run(prefix+list(args)+['--json'],env=env,capture_output=True,timeout=90)
            assert result.returncode==0,(args,result.stdout.decode(),result.stderr.decode())
            return json.loads(result.stdout)
        try:
            wait(lambda:request(api+('/config/' if engine=='caddy' else '/api/http/routers')))
            if engine=='caddy':wait(lambda:ca.exists())
            cli('proxies','add','secure','--engine',engine,'--endpoint',api,'--config-file',str(source),*flags)
            added=cli('routes','add','secure-app','--proxy','secure','--host','secure.localhost','--upstream',f'http://127.0.0.1:{backend.server_port}','--tls','internal')
            request_args=['curl','--noproxy','*','--fail','--silent','--show-error','--resolve',f'secure.localhost:{tls_port}:127.0.0.1',f'https://secure.localhost:{tls_port}/']
            untrusted=subprocess.run(request_args,capture_output=True,timeout=10);assert untrusted.returncode!=0
            trusted=subprocess.run(request_args+['--cacert',str(ca)],capture_output=True,timeout=10);assert trusted.returncode==0 and trusted.stdout==b'native-tls /',trusted.stderr
            cli('routes','disable','secure-app','--proxy','secure')
            cli('routes','enable','secure-app','--proxy','secure')
            cli('routes','remove','secure-app','--proxy','secure')
            print(engine+': native internal HTTPS issuance/configuration, trust rejection, verified routing and lifecycle passed.')
        finally:
            child.terminate()
            try:child.wait(timeout=8)
            except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
            backend.shutdown();backend.server_close()

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check-certificates',action='store_true')
    for name in ('binary','caddy','traefik'):parser.add_argument('--'+name,type=Path)
    args=parser.parse_args()
    if args.check_certificates:
        with tempfile.TemporaryDirectory(prefix='steward-certificates-') as folder:create_test_certificates(Path(folder))
        print('Portable CA/leaf generation and chain verification passed.')
    else:
        if not all((args.binary,args.caddy,args.traefik)):parser.error('--binary, --caddy and --traefik are required for native acceptance')
        exercise(args.binary.resolve(),'caddy',args.caddy.resolve());exercise(args.binary.resolve(),'traefik',args.traefik.resolve())
