import source from '../../../../../examples/static-server.py?raw';
export function GET() {
  return new Response(source, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
}
