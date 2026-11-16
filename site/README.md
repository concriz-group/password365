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

## Publishing (Cloudflare Pages)

Build command `npm run build`, output directory `dist`, root directory `site`. The domain
password365.app points at the Pages project.
