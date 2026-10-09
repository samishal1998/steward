import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: process.env.SITE_URL || 'https://samishal1998.github.io',
  base: process.env.SITE_BASE ?? '/steward',
  trailingSlash: 'always',
  integrations: [starlight({
    title: 'steward',
    description: 'A clear view of your reverse-proxy routes. A careful foundation for managing what comes next.',
    social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/samishal1998/steward' }],
    editLink: { baseUrl: 'https://github.com/samishal1998/steward/edit/main/site/' },
    customCss: ['./src/styles/custom.css'],
    sidebar: [
      { label: 'Start here', items: [
        { label: 'Installation', slug: 'guides/installation' },
        { label: 'Your first inventory', slug: 'guides/quick-start' },
      ] },
      { label: 'Read-only alpha', items: [
        { label: 'Proxy connections', slug: 'guides/connections' },
        { label: 'Route planning', slug: 'guides/planning' },
        { label: 'CLI & configuration', slug: 'reference/cli' },
        { label: 'Troubleshooting', slug: 'guides/troubleshooting' },
      ] },
      { label: 'Project', items: [
        { label: 'Architecture & roadmap', slug: 'project/architecture' },
        { label: 'Development & releases', slug: 'project/releases' },
      ] },
    ],
  })],
});
