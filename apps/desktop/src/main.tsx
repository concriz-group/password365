import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { getCurrentWindow } from "@tauri-apps/api/window";

import { App, type StatusSubscription, type Subscription } from "./App";
import { QuickAccess } from "./screens/QuickAccess";
import { createClient, type Invoke } from "./api/client";
import type { AutoTypeChoice, SshApproval, UpdateInfo } from "./api/types";
import { ClientProvider } from "./api/context";
import { I18nProvider } from "./i18n";
import { SettingsProvider } from "./settings";
import "./styles.css";

const inTauri = "__TAURI_INTERNALS__" in window;

/** Real IPC inside Tauri; an in-memory demo backend only in `vite dev` in a normal browser. */
async function pickInvoke(): Promise<Invoke> {
  if (!inTauri && import.meta.env.DEV) {
    const { createMockBackend } = await import("./dev/mockBackend");
    return createMockBackend();
  }
  return invoke;
}

const client = createClient(await pickInvoke());

/** Subscribes to a backend event; `undefined` outside Tauri (demo backend). */
function subscribeTo(event: string): StatusSubscription | undefined {
  if (!inTauri) return undefined;
  return (onChange) => {
    const unlisten = listen(event, onChange);
    return () => {
      void unlisten.then((stop) => stop());
    };
  };
}

/** Like `subscribeTo`, handing the event's payload to the callback. */
function subscribeWithPayload<T>(event: string): Subscription<T> | undefined {
  if (!inTauri) return undefined;
  return (onEvent) => {
    const unlisten = listen<T>(event, (e) => onEvent(e.payload));
    return () => {
      void unlisten.then((stop) => stop());
    };
  };
}

/** Backend emits `auto-type-choose` when the auto-type shortcut needs the user to pick an item. */
const subscribeAutoType = subscribeWithPayload<AutoTypeChoice>("auto-type-choose");

/** Backend emits `vault-status-changed` on auto-lock and on unlock from the browser extension. */
const subscribeStatus = subscribeTo("vault-status-changed");
/** Backend emits `sync-completed` after a background sync with a paired device changed items. */
const subscribeSync = subscribeTo("sync-completed");

/** Backend emits `open-item` when Quick Access asks the main window to show an item. */
const subscribeOpenItem = subscribeWithPayload<{ id: string | null }>("open-item");
/** Backend emits `ssh-approve` when a program wants to sign with an SSH key. */
const subscribeSshApprove = subscribeWithPayload<SshApproval>("ssh-approve");
/** Backend emits `update-available` when an automatic check found a new version. */
const subscribeUpdate = subscribeWithPayload<UpdateInfo>("update-available");
/** Backend emits `quick-access-shown` each time the Quick Access window appears. */
const subscribeShown = subscribeTo("quick-access-shown");

// The same page runs in the main window and in Quick Access (`?view=quick` previews it in a browser).
const quick = inTauri ? getCurrentWindow().label === "quick-access" : new URLSearchParams(location.search).get("view") === "quick";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <ClientProvider client={client}>
        <SettingsProvider>
          <I18nProvider>
            {quick ? (
              <QuickAccess subscribeShown={subscribeShown} />
            ) : (
              <App
                subscribeStatus={subscribeStatus}
                subscribeSync={subscribeSync}
                subscribeAutoType={subscribeAutoType}
                subscribeOpenItem={subscribeOpenItem}
                subscribeSshApprove={subscribeSshApprove}
                subscribeUpdate={subscribeUpdate}
              />
            )}
          </I18nProvider>
        </SettingsProvider>
      </ClientProvider>
    </StrictMode>,
  );
}
