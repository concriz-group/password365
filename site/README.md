# password365.app

The website: static pages made with [Astro](https://astro.build), English at `/`, Portuguese
at `/pt-br/`, Spanish at `/es/`.

```sh
npm install
npm run dev       # http://localhost:4321
npm run build     # → dist/
npm test          # the texts of every language
```

- Texts: `../i18n/site/<language>.json` (see `../i18n/README.md`).
- Logo and icon: copied from `../branding` before every dev or build run (`scripts/brand.mjs`);
  feature icons are the apps' own (`../icons/svg`).
- App screenshots: `public/screens/<language>/`, reduced from the iPhone and iPad App Store
  screenshots (made by the private repository's `ios/store-screenshots.sh`).
- Contact address, privacy-policy date and download links: `src/config.ts` (a download link
  left `null` shows "Coming soon").

## Publishing (Cloudflare)

```sh
npx wrangler@4.156.1 login   # once, in the browser
npm run deploy               # build, upload dist/ and serve it on password365.app and www
```

`wrangler.jsonc` describes it: Workers static assets (what Cloudflare Pages became), the 404 page
for unknown paths, and both domains (the DNS records and certificates are created by the deploy).
Free plan.

## Email

`hello@password365.app` (the contact on the Privacy and Support pages) is forwarded to the
owner's mailbox by Cloudflare Email Routing (free; rule "hello to Gmail", set up with
`wrangler email routing`). Other addresses at the domain are not delivered.
