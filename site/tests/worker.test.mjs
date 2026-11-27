// The Worker in front of the static site: one address per page (https, no www), the rest from the assets.
import { test } from "node:test";
import assert from "node:assert/strict";

import worker from "../worker/index.mjs";

const assets = { fetch: async (request) => new Response("asset " + new URL(request.url).pathname) };
const call = (url) => worker.fetch(new Request(url), { ASSETS: assets });

test("http is sent to https with a permanent redirect, keeping path and query", async () => {
  const res = await call("http://password365.app/es/download/?a=1");
  assert.equal(res.status, 301);
  assert.equal(res.headers.get("location"), "https://password365.app/es/download/?a=1");
});

test("www is sent to the bare domain", async () => {
  for (const url of ["https://www.password365.app/pt/beta/", "http://www.password365.app/pt/beta/"]) {
    const res = await call(url);
    assert.equal(res.status, 301, url);
    assert.equal(res.headers.get("location"), "https://password365.app/pt/beta/", url);
  }
});

test("the canonical address is served from the assets", async () => {
  const res = await call("https://password365.app/security/");
  assert.equal(res.status, 200);
  assert.equal(await res.text(), "asset /security/");
});

test("local previews (localhost) are not redirected", async () => {
  const res = await call("http://localhost:8787/");
  assert.equal(res.status, 200);
});
