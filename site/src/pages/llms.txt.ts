import source from '../../../llms.txt?raw';
export function GET() {
  return new Response(source, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
}
