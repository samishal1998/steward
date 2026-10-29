import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: process.env.SITE_URL || 'https://samishal1998.github.io',
  base: process.env.SITE_BASE ?? '/steward',
  trailingSlash: 'always',
  integrations: [starlight({
    title: 'steward',
    description: 'Give every route a clear way home. Native proxy tools, explicit ownership, and careful changes.',
    favicon: '/brand/waymark.svg',
    social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/samishal1998/steward' }],
    editLink: { baseUrl: 'https://github.com/samishal1998/steward/edit/main/site/' },
    customCss: ['./src/styles/custom.css'],
    sidebar: [
      { label: 'Start here', items: [
        { label: 'Installation', slug: 'guides/installation' },
        { label: 'Get started', slug: 'guides/quick-start' },
        { label: 'Shell completion', slug: 'guides/completion' },
        { label: 'Using Steward with agents', slug: 'guides/agents' },
        { label: 'Management workflows', slug: 'guides/management' },
      ] },
      { label: 'Routes & proxies', items: [
        { label: 'Proxy connections', slug: 'guides/connections' },
        { label: 'Route planning', slug: 'guides/planning' },
        { label: 'CLI & configuration', slug: 'reference/cli' },
        { label: 'Troubleshooting', slug: 'guides/troubleshooting' },
      ] },
      { label: 'Project', items: [
        { label: 'Architecture & roadmap', slug: 'project/architecture' },
        { label: 'Development & releases', slug: 'project/releases' },
        { label: 'Brand & assets', slug: 'project/brand' },
      ] },
    ],
  })],
});
