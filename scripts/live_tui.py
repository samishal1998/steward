"""Exercise keyboard TUI navigation and terminal restoration through a bounded PTY."""
import argparse
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time


def exercise(binary):
    with tempfile.TemporaryDirectory(prefix='steward-tui-',dir=os.environ.get('TMPDIR')) as folder:
        master,slave=pty.openpty()
        fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',30,120,0,0))
        original=termios.tcgetattr(slave)
        # Popen uses the platform spawn path; avoid executing Python after fork
        # in the macOS framework interpreter. Crossterm reads the terminal stdin.
        process=subprocess.Popen([str(binary),'--state-dir',folder+'/state','tui'],stdin=slave,stdout=slave,stderr=slave,start_new_session=True)
        output=bytearray()

        def read_until(text):
            deadline=time.monotonic()+10
            while time.monotonic()<deadline:
                if select.select([master],[],[],.1)[0]:
                    output.extend(os.read(master,16384))
                    assert len(output)<256*1024
                    if text in output:return
                if process.poll() is not None:break
            raise AssertionError((text,process.poll(),bytes(output[-2048:])))

        try:
            read_until(b'[Routes]');output.clear();os.write(master,b'\t');read_until(b'[Proxies]')
            output.clear();os.write(master,b'\t');read_until(b'[Services]')
            output.clear();os.write(master,b':services list\r');read_until(b'changed=false')
            os.write(master,b'q')
            deadline=time.monotonic()+10
            while process.poll() is None and time.monotonic()<deadline:
                if select.select([master],[],[],.1)[0]:os.read(master,16384)
            assert process.wait(timeout=1)==0
            assert termios.tcgetattr(slave)[3]&termios.ICANON==original[3]&termios.ICANON
            print('TUI PTY: tab navigation, checked command execution, quit and terminal restoration passed.')
        finally:
            if process.poll() is None:
                process.terminate()
                try:process.wait(timeout=2)
                except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
            os.close(master);os.close(slave)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,required=True)
    exercise(parser.parse_args().binary.resolve())
