import type { ContentMessage, Reply } from "../shared/messages";
import { startContent } from "./controller";
import { MENU_HOST_ID } from "./inline";
import { PROMPT_HOST_ID } from "./prompt";

const OPTS = { menuUrl: "chrome-extension://abc/menu.html", iconUrl: "chrome-extension://abc/i.png" };

function page(html: string) {
  document.body.innerHTML = html;
  document.getElementById(PROMPT_HOST_ID)?.remove();
  return document;
}

function sender(replies: Partial<Record<ContentMessage["kind"], Reply>>) {
  return vi.fn(async (m: ContentMessage) => replies[m.kind] ?? { ok: true as const, result: null });
}

const flush = () => new Promise((r) => setTimeout(r, 0));

describe("startContent", () => {
  it("shows a pending prompt on load and saves it", async () => {
    const doc = page("");
    const send = sender({ pending: { ok: true, result: { status: "new", username: "me" } } });
    startContent(doc, send, OPTS);
    await flush();
    expect(doc.getElementById(PROMPT_HOST_ID)).not.toBeNull();
    // Our closed root is not reachable here; use the message flow instead.
    expect(send).toHaveBeenCalledWith({ kind: "pending" });
  });

  it("reports submissions and offers the prompt right away for SPA logins", async () => {
    const doc = page(`<form><input name="user" value="me"><input type="password" value="pw"></form>`);
    doc.title = "Shop";
    let pendingCalls = 0;
    const send = vi.fn(async (m: ContentMessage): Promise<Reply> => {
      if (m.kind === "submitted") return { ok: true, result: "new" };
      if (m.kind === "pending") return { ok: true, result: pendingCalls++ ? { status: "new" } : null };
      return { ok: true, result: null };
    });
    startContent(doc, send, OPTS);
    doc.querySelector("form")!.dispatchEvent(new Event("submit"));
    await flush();
    await flush();
    expect(send).toHaveBeenCalledWith({ kind: "submitted", username: "me", password: "pw", title: "Shop" });
    expect(doc.getElementById(PROMPT_HOST_ID)).not.toBeNull();
  });

  it("does not prompt for known logins and survives background failures", async () => {
    const doc = page(`<form><input name="user" value="me"><input type="password" value="pw"></form>`);
    const send = vi.fn(async (m: ContentMessage): Promise<Reply> => {
      if (m.kind === "submitted") return { ok: true, result: "known" };
      throw new Error("extension reloaded");
    });
    startContent(doc, send, OPTS);
    doc.querySelector("form")!.dispatchEvent(new Event("submit"));
    await flush();
    expect(doc.getElementById(PROMPT_HOST_ID)).toBeNull();
  });

  it("fills on request from the background", () => {
    const doc = page(`<input name="user"><input type="password">`);
    const { onMessage, stop } = startContent(doc, sender({}), OPTS);
    expect(onMessage({ kind: "fill", username: "me", password: "pw" })).toBe(true);
    expect((doc.querySelector("input[type=password]") as HTMLInputElement).value).toBe("pw");
    expect(onMessage({ kind: "other" } as never)).toBe(false);
    page("<p>no form</p>");
    expect(onMessage({ kind: "fill", username: "me", password: "pw" })).toBe(false);
    stop();
  });

  it("closes the menu on fill, suggests sign-up passwords, closes and resizes on request", () => {
    const doc = page(`<form><input name="user"><input type="password"><input type="password"></form>`);
    const { onMessage, stop } = startContent(doc, sender({}), OPTS);
    const user = doc.querySelector<HTMLInputElement>("input[name=user]")!;
    user.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
    expect(doc.getElementById(MENU_HOST_ID)).not.toBeNull();
    expect(onMessage({ kind: "resize-menu", height: 150 })).toBe(true);
    expect(onMessage({ kind: "fill-new-password", password: "N3w!" })).toBe(true);
    expect(doc.getElementById(MENU_HOST_ID)).toBeNull();
    const values = [...doc.querySelectorAll<HTMLInputElement>("input[type=password]")].map((p) => p.value);
    expect(values).toEqual(["N3w!", "N3w!"]);
    user.dispatchEvent(new FocusEvent("focusout", { bubbles: true }));
    doc.querySelector<HTMLInputElement>("input[type=password]")!.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
    expect(onMessage({ kind: "close-menu" })).toBe(true);
    expect(doc.getElementById(MENU_HOST_ID)).toBeNull();
    stop();
  });
});

describe("suggest from the context menu", () => {
  it("fills the field that was right-clicked, and nothing without one", () => {
    const doc = page(`<form><input name="user"><input type="password" id="p"><input type="password" id="c"></form>`);
    const { onMessage, stop } = startContent(doc, sender({}), OPTS);
    expect(onMessage({ kind: "suggest-at-target", password: "S1!" })).toBe(false);
    const p = doc.getElementById("p") as HTMLInputElement;
    p.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true }));
    expect(onMessage({ kind: "suggest-at-target", password: "S1!" })).toBe(true);
    expect((doc.getElementById("c") as HTMLInputElement).value).toBe("S1!");
    doc.body.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true })); // not a field
    expect(onMessage({ kind: "suggest-at-target", password: "S2!" })).toBe(false);
    stop();
  });

  it("opens the menu with keyboard focus for the fill shortcut", () => {
    const doc = page(`<form><input name="user"><input type="password"></form>`);
    const { onMessage, stop } = startContent(doc, sender({}), OPTS);
    expect(onMessage({ kind: "open-menu" })).toBe(true);
    expect(onMessage({ kind: "open-passkey" })).toBe(true);
    expect(onMessage({ kind: "passkey-fields", active: false })).toBe(true);
    expect(doc.activeElement).toBe(doc.getElementById(MENU_HOST_ID));
    stop();
    const bare = page(`<p>no form</p>`);
    const other = startContent(bare, sender({}), OPTS);
    expect(other.onMessage({ kind: "open-menu" })).toBe(false);
    other.stop();
  });

  it("fills the address or payment form the background sends values for", () => {
    const doc = page(`<form><input autocomplete="cc-number"><input autocomplete="cc-exp" placeholder="MM/YY"></form>`);
    const content = startContent(doc, sender({}), OPTS);
    const values = { "cc-number": "4111", "cc-exp-month": "04", "cc-exp-year": "2029" };
    expect(content.onMessage({ kind: "fill-profile", profile: "card", values })).toBe(true);
    expect([...doc.querySelectorAll("input")].map((i) => i.value)).toEqual(["4111", "04/29"]);
    expect(content.onMessage({ kind: "fill-profile", profile: "identity", values })).toBe(false);
    content.stop();
  });
});
