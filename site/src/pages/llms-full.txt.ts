import source from '../../../docs/AGENT_GUIDE.md?raw';
export function GET() {
  return new Response(source, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
}
