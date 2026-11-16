// Firefox reads the same extension with a few manifest differences.

/** Fixed add-on id: native messaging names it in the host's allowed_extensions. */
export const FIREFOX_ID = "extension@password365.app";

/** The Chromium manifest adapted for Firefox (MV3, 128+). */
export function firefoxManifest(chromium) {
  const { key: _key, background, ...rest } = chromium;
  return {
    ...rest,
    // Firefox runs MV3 background scripts as event pages, not service workers.
    background: { scripts: [background.service_worker], type: background.type },
    browser_specific_settings: { gecko: { id: FIREFOX_ID, strict_min_version: "128.0" } },
  };
}
