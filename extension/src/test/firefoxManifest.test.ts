// @ts-expect-error plain JS build helper
import { FIREFOX_ID, firefoxManifest } from "../../scripts/firefox.mjs";

describe("Firefox manifest", () => {
  it("swaps the service worker for background scripts, drops the Chrome key and names the add-on", () => {
    const out = firefoxManifest({ name: "P", key: "abc", background: { service_worker: "background.js", type: "module" }, permissions: ["nativeMessaging"] });
    expect(out.key).toBeUndefined();
    expect(out.background).toEqual({ scripts: ["background.js"], type: "module" });
    expect(out.browser_specific_settings.gecko.id).toBe(FIREFOX_ID);
    expect(out.permissions).toEqual(["nativeMessaging"]);
  });
});
