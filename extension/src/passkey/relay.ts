// Isolated content script at document start: carries the page script's
// passkey requests to the background and its answers back. The site is never
// taken from the message; the background reads the tab's address.
import type { ContentMessage, Reply } from "../shared/messages";
import type { PageReply } from "./page";

const KINDS = {
  create: "passkey-create",
  get: "passkey-get",
  conditional: "passkey-conditional",
  abort: "passkey-abort",
} as const;

export function startRelay(win: Window, send: (message: ContentMessage) => Promise<Reply>): void {
  win.addEventListener("message", (e: MessageEvent) => {
    const data = e.data as { password365Passkey?: unknown; id?: unknown; options?: unknown } | null;
    const asked = data?.password365Passkey;
    if (e.source !== win || typeof asked !== "string" || !(asked in KINDS) || typeof data?.id !== "number") return;
    const kind = KINDS[asked as keyof typeof KINDS];
    const id = data.id;
    if (kind === "passkey-abort") {
      void send({ kind }).catch(() => undefined);
      return;
    }
    void send({ kind, options: data.options } as ContentMessage)
      .then((reply): PageReply => (reply?.ok ? (reply.result as PageReply) : { fallback: true }))
      .catch((): PageReply => ({ fallback: true }))
      .then((reply) => win.postMessage({ password365PasskeyReply: id, ...reply }, win.location.origin));
  });
}
