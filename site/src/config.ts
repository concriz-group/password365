/** Facts the pages share (one place to change them). */
export const site = {
  domain: "password365.app",
  email: "hello@password365.app",
  privacyUpdated: "2026-11-16",
  /**
   * Installers come from the newest GitHub release, under stable names. Set `released` to true
   * once the first release is out (until then the Download page says "Coming soon").
   */
  released: true,
  latest: "https://github.com/remofurlanetto/password365/releases/latest/download/",
  /** The public repository (code, specifications, issues). */
  repo: "https://github.com/remofurlanetto/password365",
  /** Store pages, once listed (null: not yet). */
  stores: {
    chrome: null as string | null,
    firefox: null as string | null,
    edge: null as string | null,
    appStore: null as string | null,
  },
};

/** Where to download a desktop installer, or null before the first release. */
export function download(
  file:
    | "Password365-macOS.dmg"
    | "Password365-Windows-setup.exe"
    | "Password365-Linux.AppImage"
    | "Password365-Linux.deb"
    | "Password365-extension-Chromium.zip"
    | "Password365-extension-Firefox.zip",
): string | null {
  return site.released ? site.latest + file : null;
}
