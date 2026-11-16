import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

import type { Settings } from "../api/types";
import { useClient } from "../api/context";

/** The same defaults as the core (`settings.rs`). */
export const DEFAULT_SETTINGS: Settings = {
  auto_lock_minutes: 10,
  language: "",
  lock_on_sleep: true,
  clipboard_clear_seconds: 90,
  theme: "system",
  density: "comfortable",
  zoom_percent: 100,
  sidebar_categories: true,
  sidebar_tags: true,
  default_vault: "",
  open_to: "all_items",
  start_at_login: false,
  show_wifi_qr: true,
  ssh_agent: false,
  check_breached_passwords: false,
  shortcut_quick_access: typeof navigator !== "undefined" && /Mac/.test(navigator.platform) ? "Shift+Super+Space" : "Ctrl+Shift+Space",
  shortcut_auto_type: "Ctrl+Alt+A",
  shortcut_lock: "",
  region: "",
};

interface SettingsState {
  settings: Settings;
  /** Loaded from the backend (until then the defaults are shown). */
  loaded: boolean;
  /** Saves a change; the screen follows right away. */
  update: (change: Partial<Settings>) => Promise<void>;
}

// Without a provider (tests, previews) the defaults are all there is: "loaded".
const SettingsContext = createContext<SettingsState>({
  settings: DEFAULT_SETTINGS,
  loaded: true,
  update: async () => {},
});

/** Applies theme, density and zoom to the page. */
export function applyToPage(settings: Settings, root: HTMLElement = document.documentElement): void {
  if (settings.theme === "system") delete root.dataset.theme;
  else root.dataset.theme = settings.theme;
  root.dataset.density = settings.density;
  root.style.setProperty("zoom", settings.zoom_percent === 100 ? "" : `${settings.zoom_percent}%`);
}

/** Loads this device's preferences and keeps the page in step with them. */
export function SettingsProvider({ children }: { children: ReactNode }) {
  const client = useClient();
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    void client
      .settings()
      .then((s) => {
        setSettings({ ...DEFAULT_SETTINGS, ...(s ?? {}) });
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
  }, [client]);

  useEffect(() => applyToPage(settings), [settings]);

  const update = useCallback(
    async (change: Partial<Settings>) => {
      const next = { ...settings, ...change };
      await client.updateSettings(next);
      setSettings(next);
    },
    [client, settings],
  );

  const value = useMemo(() => ({ settings, loaded, update }), [settings, loaded, update]);
  return <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>;
}

/** This device's preferences and how to change them. */
export const useSettings = (): SettingsState => useContext(SettingsContext);
