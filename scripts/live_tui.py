"""Exercise keyboard TUI navigation and native terminal restoration through a PTY."""
import argparse
import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import tempfile
import termios
import time

def exercise(binary):
    with tempfile.TemporaryDirectory(prefix='steward-tui-',dir=os.environ.get('TMPDIR')) as folder:
        pid,terminal=pty.fork()
        if pid==0:
            os.execv(str(binary),[str(binary),'--state-dir',folder+'/state','tui'])
        fcntl.ioctl(terminal,termios.TIOCSWINSZ,struct.pack('HHHH',30,120,0,0))
        output=bytearray()
        def read_until(text):
            deadline=time.monotonic()+10
            while time.monotonic()<deadline:
                if select.select([terminal],[],[],.1)[0]:
                    output.extend(os.read(terminal,16384))
                    assert len(output)<256*1024
                    if text in output:return
            raise AssertionError((text,bytes(output[-2048:])))
        try:
            read_until(b'[Routes]');output.clear();os.write(terminal,b'\t');read_until(b'[Proxies]')
            output.clear();os.write(terminal,b'\t');read_until(b'[Services]')
            output.clear();os.write(terminal,b':services list\r');read_until(b'changed=false')
            os.write(terminal,b'q')
            deadline=time.monotonic()+10
            while time.monotonic()<deadline:
                result,status=os.waitpid(pid,os.WNOHANG)
                if result:
                    assert os.waitstatus_to_exitcode(status)==0
                    assert termios.tcgetattr(terminal)[3]&termios.ICANON
                    print('TUI PTY: tab navigation, checked command execution, quit and terminal restoration passed.')
                    return
                time.sleep(.05)
            raise AssertionError('TUI did not exit')
        finally:
            try:os.kill(pid,signal.SIGTERM);os.waitpid(pid,0)
            except ProcessLookupError:pass
            except ChildProcessError:pass
            os.close(terminal)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,required=True)
    exercise(parser.parse_args().binary.resolve())
