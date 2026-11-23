/** Structured data (schema.org JSON-LD) for the pages: what the product is, and the questions answered. */
import { site } from "./config";

const ORIGIN = "https://" + site.domain;

export const organization = {
  "@type": "Organization",
  "@id": `${ORIGIN}/#organization`,
  name: "Password365",
  alternateName: ["Password 365"],
  url: ORIGIN + "/",
  logo: `${ORIGIN}/brand/app-icon-1024.png`,
  sameAs: [site.repo],
  email: site.email,
};

export const website = (description: string, language: string) => ({
  "@type": "WebSite",
  "@id": `${ORIGIN}/#website`,
  url: ORIGIN + "/",
  name: "Password365",
  alternateName: ["Password 365"],
  description,
  inLanguage: language,
  publisher: { "@id": `${ORIGIN}/#organization` },
});

/** The app itself (free on computers; the phone app is a subscription, which the description says). */
export const softwareApplication = (description: string, language: string) => ({
  "@type": "SoftwareApplication",
  "@id": `${ORIGIN}/#app`,
  name: "Password365",
  alternateName: ["Password 365"],
  applicationCategory: "SecurityApplication",
  operatingSystem: "macOS, Windows, Linux",
  description,
  inLanguage: language,
  url: ORIGIN + "/",
  downloadUrl: `${ORIGIN}/download/`,
  license: "https://www.gnu.org/licenses/gpl-3.0.html",
  codeRepository: site.repo,
  offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
  publisher: { "@id": `${ORIGIN}/#organization` },
});

export const faqPage = (items: { q: string; a: string }[]) => ({
  "@type": "FAQPage",
  mainEntity: items.map((i) => ({
    "@type": "Question",
    name: i.q,
    acceptedAnswer: { "@type": "Answer", text: i.a },
  })),
});

export const article = (o: { title: string; description: string; url: string; language: string; modified: string }) => ({
  "@type": "TechArticle",
  headline: o.title,
  description: o.description,
  url: o.url,
  inLanguage: o.language,
  dateModified: o.modified,
  author: { "@id": `${ORIGIN}/#organization` },
  publisher: { "@id": `${ORIGIN}/#organization` },
  mainEntityOfPage: o.url,
});

/** One <script> body: several objects in a graph, safe to place inside HTML. */
export const graph = (nodes: object[]) =>
  JSON.stringify({ "@context": "https://schema.org", "@graph": nodes }).replace(/</g, "\\u003c");
