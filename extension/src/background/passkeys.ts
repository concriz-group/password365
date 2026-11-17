// Passkey requests from pages, one per tab. The user answers in the
// extension's own frame (menu.html in passkey mode), which the page can
// neither read nor click by script; the site always comes from the tab's
// real address.
import type { CreateOptions, GetOptions } from "../passkey/codec";
import type { PageReply } from "../passkey/page";
import type { PasskeyChoice, PasskeyPrompt } from "../shared/messages";
import { Password365Error, type AppStatus } from "../shared/protocol";
import type { TabsApi } from "./actions";
import type { Bridge } from "./bridge";

/** How long a passkey prompt waits for the user. */
export const PASSKEY_TIMEOUT_MS = 2 * 60_000;

/** `choices` is `null` until the app is unlocked (then they are looked up). */
type Request = { kind: "create"; options: CreateOptions } | { kind: "get"; options: GetOptions; choices: PasskeyChoice[] | null };

interface Pending {
  request: Request;
  pageUrl: string;
  resolve: (reply: PageReply) => void;
  timer: ReturnType<typeof setTimeout>;
}

interface Deps {
  bridge: Pick<Bridge, "request">;
  tabs: TabsApi;
  /** Opens the toolbar popup (where the master password is typed). */
  openPopup?: () => Promise<void>;
  timeoutMs?: number;
}

const FALLBACK: PageReply = { fallback: true };
const CANCELLED: PageReply = { error: { name: "NotAllowedError", message: "The operation either timed out or was not allowed." } };

const siteOf = (pageUrl: string) => new URL(pageUrl).hostname;

export function createPasskeys({ bridge, tabs, openPopup = async () => {}, timeoutMs = PASSKEY_TIMEOUT_MS }: Deps) {
  const pending = new Map<number, Pending>();
  /**
   * "Conditional" sign-ins: the page waits (with no time limit) while its
   * passkeys are offered in the inline menu of the username field.
   */
  const offered = new Map<number, { pageUrl: string; options: GetOptions; choices: PasskeyChoice[]; resolve: (reply: PageReply) => void }>();
  /**
   * Answers waiting for the user to be verified (the site requires it and the
   * app was not unlocked recently): retried with the master password typed in
   * the toolbar popup, never in the page.
   */
  const verifying = new Map<number, (password: string) => Promise<void>>();

  /**
   * Runs `answer`; when the app needs the user verified, keeps it for
   * [`verify`], opens the popup and returns `"verify"`.
   */
  async function verified(tabId: number, answer: (password?: string) => Promise<void>): Promise<true | "verify"> {
    try {
      await answer();
      return true;
    } catch (error) {
      if (!(error instanceof Password365Error) || error.code !== "verification_needed") throw error;
      verifying.set(tabId, answer);
      await openPopup();
      return "verify";
    }
  }

  /** The master password typed in the popup: answers the waiting request. */
  async function verify(tabId: number, password: string): Promise<boolean> {
    const answer = verifying.get(tabId);
    if (!answer) return false;
    await answer(password);
    verifying.delete(tabId);
    return true;
  }

  function stopOffering(tabId: number, reply: PageReply | null) {
    const current = offered.get(tabId);
    if (!current) return;
    offered.delete(tabId);
    verifying.delete(tabId);
    if (reply) current.resolve(reply);
    void tabs.send(tabId, { kind: "passkey-fields", active: false }).catch(() => undefined);
  }

  /** A conditional get: offered in the menu when this vault has passkeys for the site. */
  async function conditional(tabId: number, pageUrl: string, options: GetOptions): Promise<PageReply> {
    const status = await bridge.request<AppStatus>({ type: "status" }).catch(() => null);
    if (!status?.unlocked) return FALLBACK;
    const choices = await choicesFor(pageUrl, options);
    if (choices.length === 0) return FALLBACK;
    stopOffering(tabId, FALLBACK);
    const reply = new Promise<PageReply>((resolve) => offered.set(tabId, { pageUrl, options, choices, resolve }));
    await tabs.send(tabId, { kind: "passkey-fields", active: true }).catch(() => stopOffering(tabId, FALLBACK));
    return reply;
  }

  /** The passkeys the inline menu offers for this tab, if a conditional request waits. */
  function offeredChoices(tabId: number): PasskeyChoice[] | undefined {
    return offered.get(tabId)?.choices;
  }

  /** The user picked a passkey in the inline menu. */
  async function autofill(tabId: number, itemId: string): Promise<boolean | "verify"> {
    const current = offered.get(tabId);
    if (!current?.choices.some((c) => c.item_id === itemId)) return false;
    await tabs.send(tabId, { kind: "close-menu" }).catch(() => undefined);
    return verified(tabId, async (password) => {
      const assertion = await bridge.request({
        type: "passkey_assert",
        item_id: itemId,
        page_url: current.pageUrl,
        challenge: current.options.challenge,
        user_verification: current.options.user_verification,
        password,
      });
      stopOffering(tabId, { result: assertion as never });
    });
  }

  function finish(tabId: number, reply: PageReply) {
    const current = pending.get(tabId);
    if (!current) return;
    pending.delete(tabId);
    verifying.delete(tabId);
    clearTimeout(current.timer);
    current.resolve(reply);
  }

  /** A page's create/get: resolves when the user has answered (or it falls back). */
  async function request(tabId: number, pageUrl: string, kind: "create" | "get", options: CreateOptions | GetOptions): Promise<PageReply> {
    const status = await bridge.request<AppStatus>({ type: "status" }).catch(() => null);
    // Without the app (or an account), the browser's own passkeys are used.
    // A locked app asks to be unlocked in the prompt, like 1Password.
    if (!status?.initialized) return FALLBACK;
    let request: Request;
    if (kind === "get") {
      const get = options as GetOptions;
      const choices = status.unlocked ? await choicesFor(pageUrl, get) : null;
      if (choices?.length === 0) return FALLBACK;
      request = { kind, options: get, choices };
    } else {
      request = { kind, options: options as CreateOptions };
    }
    finish(tabId, FALLBACK);
    const reply = new Promise<PageReply>((resolve) => {
      const timer = setTimeout(() => finish(tabId, CANCELLED), timeoutMs);
      pending.set(tabId, { request, pageUrl, resolve, timer });
    });
    try {
      await tabs.send(tabId, { kind: "open-passkey" });
    } catch {
      finish(tabId, FALLBACK);
    }
    return reply;
  }

  function choicesFor(pageUrl: string, get: GetOptions): Promise<PasskeyChoice[]> {
    return bridge
      .request<PasskeyChoice[]>({ type: "passkey_options", page_url: pageUrl, rp_id: get.rp_id, allow: get.allow })
      .catch(() => []);
  }

  /** What the prompt shows for this tab (`null`: nothing waiting). */
  async function prompt(tabId: number): Promise<PasskeyPrompt | null> {
    const current = pending.get(tabId);
    if (!current) return null;
    const site = siteOf(current.pageUrl);
    const status = await bridge.request<AppStatus>({ type: "status" }).catch(() => null);
    if (!status?.unlocked) return { kind: "locked", site };
    if (verifying.has(tabId)) return { kind: "verify", site };
    const { request } = current;
    if (request.kind === "create") return { kind: "create", site, user_name: request.options.user_name };
    if (request.choices === null) {
      // Just unlocked: now we can tell whether this vault has a passkey for the site.
      request.choices = await choicesFor(current.pageUrl, request.options);
      if (request.choices.length === 0) {
        await other(tabId);
        return null;
      }
    }
    return { kind: "get", site, choices: request.choices };
  }

  /** "Unlock Password365": the master password is typed in the popup, never in the page. */
  async function unlock(): Promise<boolean> {
    await openPopup();
    return true;
  }

  /** The user chose: save the passkey, or sign in with `itemId`'s. */
  async function choose(tabId: number, itemId?: string): Promise<boolean | "verify"> {
    const current = pending.get(tabId);
    if (!current) return false;
    // The tab must still be on the site that asked. Without the "tabs"
    // permission the browser may not tell; the site of the request itself
    // came from the browser (the sender), and a page that navigated away no
    // longer receives the answer.
    const now = await tabs.url(tabId).catch(() => undefined);
    if (now && URL.canParse(now) && new URL(now).origin !== new URL(current.pageUrl).origin) {
      finish(tabId, CANCELLED);
      return false;
    }
    const { request, pageUrl } = current;
    if (request.kind === "get" && !request.choices?.some((c) => c.item_id === itemId)) return false;
    const answered = await verified(tabId, async (password) => {
      if (request.kind === "create") {
        const { rp_name, exclude, ...wanted } = request.options;
        const { registration } = await bridge.request<{ registration: PageReply & object }>({
          type: "passkey_register",
          page_url: pageUrl,
          title: rp_name,
          request: wanted,
          exclude,
          password,
        });
        finish(tabId, { result: registration as never });
      } else {
        const assertion = await bridge.request({
          type: "passkey_assert",
          item_id: itemId!,
          page_url: pageUrl,
          challenge: request.options.challenge,
          user_verification: request.options.user_verification,
          password,
        });
        finish(tabId, { result: assertion as never });
      }
      await tabs.send(tabId, { kind: "close-menu" }).catch(() => undefined);
    });
    return answered;
  }

  /** "Use another device or method": the browser takes over. */
  async function other(tabId: number): Promise<boolean> {
    finish(tabId, FALLBACK);
    await tabs.send(tabId, { kind: "close-menu" }).catch(() => undefined);
    return true;
  }

  return {
    request,
    prompt,
    choose,
    other,
    unlock,
    verify,
    /** Whether the tab's passkey answer waits for the master password. */
    verifying: (tabId: number) => verifying.has(tabId),
    cancel: (tabId: number) => finish(tabId, CANCELLED),
    /** Whether the tab's page waits for an answer in the passkey prompt. */
    waiting: (tabId: number) => pending.has(tabId),
    conditional,
    offeredChoices,
    autofill,
    /** The page gave up its conditional request. */
    abort: (tabId: number) => stopOffering(tabId, null),
  };
}
