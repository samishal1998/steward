import source from '../../assets/waymark.svg?raw';
export function GET() {
  return new Response(source, { headers: { 'Content-Type': 'image/svg+xml; charset=utf-8' } });
}
