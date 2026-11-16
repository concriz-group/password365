import {
  DEFAULT_GENERATE,
  type FillCredentials,
  type Generated,
  Password365Error,
} from "../shared/protocol";
import type { TabMessage } from "../shared/messages";
import type { Bridge } from "./bridge";

export interface TabsApi {
  /** Trusted URL of a tab, as reported by the browser. */
  url(tabId: number): Promise<string | undefined>;
  send(tabId: number, message: TabMessage): Promise<void>;
}

interface Deps {
  bridge: Pick<Bridge, "request">;
  tabs: TabsApi;
}

/** Actions shared by every entry point (popup, inline menu, shortcut, context menu). */
export function createActions({ bridge, tabs }: Deps) {
  /** The tab's URL from the browser — never one supplied by a page. */
  async function tabUrl(tabId: number): Promise<string> {
    const url = await tabs.url(tabId);
    if (!url) throw new Password365Error("no_page", "This page cannot be filled");
    return url;
  }

  /** Asks the app for one login's credentials (it re-checks the site) and fills the tab. */
  async function fillTab(tabId: number, itemId: string): Promise<true> {
    const page_url = await tabUrl(tabId);
    const creds = await bridge.request<FillCredentials>({ type: "fill", item_id: itemId, page_url });
    await tabs.send(tabId, { kind: "fill", ...creds });
    return true;
  }

  function generate() {
    return bridge.request<Generated>({ type: "generate", request: DEFAULT_GENERATE });
  }

  return { tabUrl, fillTab, generate };
}

export type Actions = ReturnType<typeof createActions>;
