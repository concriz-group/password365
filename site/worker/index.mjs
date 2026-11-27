// In front of the static site (Cloudflare Workers static assets): every page has one address,
// https://password365.app/…. Requests over http or to www get a permanent redirect there, so search
// engines index one copy of each page; everything else is served from the built files.

const CANONICAL_HOST = "password365.app";

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const ours = url.hostname === CANONICAL_HOST || url.hostname === "www." + CANONICAL_HOST;
    if (ours && (url.protocol === "http:" || url.hostname !== CANONICAL_HOST)) {
      url.protocol = "https:";
      url.hostname = CANONICAL_HOST;
      url.port = "";
      return Response.redirect(url.toString(), 301);
    }
    return env.ASSETS.fetch(request);
  },
};
