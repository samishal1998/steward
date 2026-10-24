import source from '../../../../../examples/service-stack.json?raw';
export function GET() {
  return new Response(source, { headers: { 'Content-Type': 'application/json; charset=utf-8' } });
}
