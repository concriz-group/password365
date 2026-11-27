import type { APIRoute } from "astro";

import { guides, pageModified, pages } from "../guides";
import { href, languages } from "../i18n";

/** Every indexable page in every language, each with its translations (the pairing pages are not listed). */
export const GET: APIRoute = ({ site }) => {
  const origin = (site ?? new URL("https://password365.app")).toString().replace(/\/$/, "");
  const all: [string, string][] = [...pages.map((p) => [p, pageModified[p]] as [string, string]), ...guides.map((g) => [g.slug, g.modified] as [string, string])];
  const url = (tag: string, page: string) => origin + href(tag, page);
  const entries = all.flatMap(([page, modified]) =>
    languages.map((l) => {
      const alternates = [
        ...languages.map((o) => `<xhtml:link rel="alternate" hreflang="${o.tag}" href="${url(o.tag, page)}"/>`),
        `<xhtml:link rel="alternate" hreflang="x-default" href="${url("en", page)}"/>`,
      ].join("");
      return `<url><loc>${url(l.tag, page)}</loc><lastmod>${modified}</lastmod>${alternates}</url>`;
    }),
  );
  const body = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">${entries.join("")}</urlset>\n`;
  return new Response(body, { headers: { "Content-Type": "application/xml; charset=utf-8" } });
};
