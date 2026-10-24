import source from '../../../../schemas/export.schema.json?raw';
export function GET() {
  return new Response(source, { headers: { 'Content-Type': 'application/json; charset=utf-8' } });
}
