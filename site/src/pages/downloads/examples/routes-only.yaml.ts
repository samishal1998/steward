import content from '../../../../../examples/routes-only.yaml?raw';
export const prerender = true;
export const GET = () => new Response(content, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
