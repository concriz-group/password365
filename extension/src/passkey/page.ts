// Runs in the page's own JavaScript world (MAIN) at document start: answers
// `navigator.credentials.create/get` for passkeys with Password365, like
// 1Password. Everything else — and anything the user declines or the app
// cannot do — goes to the browser's own implementation unchanged.
//
// The page can see and fake these window messages, but that gains it
// nothing: the extension takes the site from the tab's real address, and the
// user confirms every passkey in the extension's own frame.
import { type Assertion, assertionCredential, createOptions, getOptions, type Registration, registrationCredential } from "./codec";

/** What the extension answers. */
export type PageReply =
  | { fallback: true }
  | { result: Registration | Assertion }
  | { error: { name: string; message: string } };

export function installPasskeys(win: Window & typeof globalThis): void {
  const credentials = win.navigator.credentials;
  if (!credentials || typeof credentials.create !== "function") return;
  const originalCreate = credentials.create.bind(credentials);
  const originalGet = credentials.get.bind(credentials);
  const waiting = new Map<number, (reply: PageReply) => void>();
  let next = 0;

  win.addEventListener("message", (e: MessageEvent) => {
    const data = e.data as ({ password365PasskeyReply?: unknown } & PageReply) | null;
    if (e.source !== win || typeof data?.password365PasskeyReply !== "number") return;
    const resolve = waiting.get(data.password365PasskeyReply);
    waiting.delete(data.password365PasskeyReply);
    resolve?.(data);
  });

  function ask(kind: "create" | "get" | "conditional", options: unknown, signal?: AbortSignal | null): Promise<PageReply> {
    return new Promise((resolve, reject) => {
      if (signal?.aborted) return reject(new win.DOMException("The operation was aborted.", "AbortError"));
      const id = ++next;
      waiting.set(id, resolve);
      signal?.addEventListener("abort", () => {
        waiting.delete(id);
        // Tell the extension, so it stops offering this request's passkeys.
        win.postMessage({ password365Passkey: "abort", id }, win.location.origin);
        reject(new win.DOMException("The operation was aborted.", "AbortError"));
      });
      win.postMessage({ password365Passkey: kind, id, options }, win.location.origin);
    });
  }

  function settle<T>(reply: PageReply, build: (r: never) => T, fallback: () => Promise<T>): Promise<T> | T {
    if ("result" in reply) return build(reply.result as never);
    if ("error" in reply) throw new win.DOMException(reply.error.message, reply.error.name);
    return fallback();
  }

  credentials.create = async function create(options?: CredentialCreationOptions) {
    if (!options?.publicKey) return originalCreate(options);
    const reply = await ask("create", createOptions(options.publicKey, win.location.hostname), options.signal);
    return settle(reply, (r: Registration) => registrationCredential(r, win as never), () => originalCreate(options));
  };
  credentials.get = async function get(options?: CredentialRequestOptions) {
    if (!options?.publicKey) return originalGet(options);
    // "conditional": passkeys offered in the username field's list. Ours appear
    // in Password365's menu there; without any, the browser's list is used.
    const kind = options.mediation === "conditional" ? "conditional" : "get";
    const reply = await ask(kind, getOptions(options.publicKey, win.location.hostname), options.signal);
    return settle(reply, (a: Assertion) => assertionCredential(a, win as never), () => originalGet(options));
  };
}
