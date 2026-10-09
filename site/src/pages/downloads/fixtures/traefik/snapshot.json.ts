import content from '../../../../../../fixtures/traefik/snapshot.json?raw';
export const prerender = true;
export const GET = () => new Response(content, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
