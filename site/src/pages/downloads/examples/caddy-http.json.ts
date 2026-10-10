import source from '../../../../../examples/caddy-http.json?raw';
export function GET() {
  return new Response(source, { headers: { 'Content-Type': 'application/json; charset=utf-8' } });
}
