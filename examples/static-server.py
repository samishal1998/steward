"""Small foreground static server for Steward's cross-platform service example."""
import argparse
import http.server
import ipaddress
import socket
import socketserver

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--port', type=int, default=3000)
parser.add_argument('--bind', type=ipaddress.ip_address, default=ipaddress.ip_address('127.0.0.1'))
args = parser.parse_args()
if not 1 <= args.port <= 65535:
    parser.error('port must be between 1 and 65535')


class Server(http.server.ThreadingHTTPServer):
    address_family = socket.AF_INET6 if args.bind.version == 6 else socket.AF_INET

    def server_bind(self):
        # HTTPServer normally reverse-resolves its bound address during startup.
        # This numeric-address example needs no DNS or macOS mDNS permission.
        socketserver.TCPServer.server_bind(self)
        self.server_name = str(args.bind)
        self.server_port = self.server_address[1]


with Server((str(args.bind), args.port), http.server.SimpleHTTPRequestHandler) as server:
    server.serve_forever()
