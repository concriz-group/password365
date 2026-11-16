import { findLoginFields } from "./detect";
import { fillLogin, setValue } from "./fill";
import { PROMPT_HOST_ID, showSavePrompt } from "./prompt";
import { DEDUPE_MS, watchSubmissions } from "./submit";

function page(html: string) {
  document.body.innerHTML = html;
  return document;
}

describe("findLoginFields", () => {
  it("finds username and password in a login form", () => {
    const doc = page(`<form><input name="q"><input type="email" name="email"><input type="password"></form>`);
    const f = findLoginFields(doc)!;
    expect(f.username?.name).toBe("email");
    expect(f.form).not.toBeNull();
    expect(f.isSignup).toBe(false);
  });

  it("prefers inputs that look like usernames, else the nearest text input", () => {
    const doc = page(`<input id="login-user"><input id="other"><input type="password">`);
    expect(findLoginFields(doc)!.username?.id).toBe("login-user");
    page(`<input id="a"><input id="b"><input type="password">`);
    expect(findLoginFields(document)!.username?.id).toBe("b");
  });

  it("detects sign-up forms", () => {
    page(`<form><input name="user"><input type="password"><input type="password"></form>`);
    expect(findLoginFields(document)!.isSignup).toBe(true);
    page(`<form><input type="password" autocomplete="new-password"></form>`);
    expect(findLoginFields(document)!.isSignup).toBe(true);
  });

  it("ignores disabled/hidden fields and pages without passwords", () => {
    expect(findLoginFields(page(`<input name="user">`))).toBeNull();
    expect(findLoginFields(page(`<input type="password" disabled>`))).toBeNull();
    const f = findLoginFields(page(`<input name="user" readonly><input type="password">`))!;
    expect(f.username).toBeNull();
    expect(f.form).toBeNull();
  });
});

describe("fill", () => {
  it("sets values and fires input/change events", () => {
    const doc = page(`<input name="user"><input type="password">`);
    const f = findLoginFields(doc)!;
    const events: string[] = [];
    f.password.addEventListener("input", () => events.push("input"));
    f.password.addEventListener("change", () => events.push("change"));
    expect(fillLogin(f, { username: "me", password: "pw" })).toBe(true);
    expect(f.username!.value).toBe("me");
    expect(f.password.value).toBe("pw");
    expect(events).toEqual(["input", "change"]);
  });

  it("fills nothing when there is nothing to fill", () => {
    const f = findLoginFields(page(`<input type="password">`))!;
    expect(fillLogin(f, { username: "me", password: null })).toBe(false);
  });

  it("setValue falls back when no native setter exists", () => {
    const input = document.createElement("input");
    const spy = vi.spyOn(Object, "getOwnPropertyDescriptor").mockReturnValueOnce(undefined);
    setValue(input, "x");
    expect(input.value).toBe("x");
    spy.mockRestore();
  });
});

describe("watchSubmissions", () => {
  function setupForm() {
    const doc = page(`<form id="f"><input name="user"><input type="password"><button>Go</button>
      <button type="button" id="other">Show</button></form>`);
    const f = findLoginFields(doc)!;
    f.username!.value = " me ";
    f.password.value = "pw";
    let time = 0;
    const onSubmit = vi.fn();
    const stop = watchSubmissions(doc, onSubmit, () => time, () => true);
    return { doc, onSubmit, stop, advance: (ms: number) => (time += ms), f };
  }

  it("captures submit events once (deduplicated with the button click)", () => {
    const { doc, onSubmit } = setupForm();
    (doc.querySelector("button") as HTMLButtonElement).click();
    doc.getElementById("f")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith({ username: "me", password: "pw" });
  });

  it("captures Enter in the password field, ignores other buttons, keys and empty passwords", () => {
    const { doc, onSubmit, f, advance } = setupForm();
    doc.getElementById("other")!.click();
    f.username!.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    f.password.dispatchEvent(new KeyboardEvent("keydown", { key: "a", bubbles: true }));
    expect(onSubmit).not.toHaveBeenCalled();
    f.password.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    expect(onSubmit).toHaveBeenCalledTimes(1);
    advance(DEDUPE_MS + 1);
    f.password.value = "";
    f.password.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    doc.body.click();
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it("reports again after the dedupe window and stops when asked", () => {
    const { doc, onSubmit, stop, advance } = setupForm();
    const form = doc.getElementById("f")!;
    form.dispatchEvent(new Event("submit"));
    advance(DEDUPE_MS + 1);
    form.dispatchEvent(new Event("submit"));
    expect(onSubmit).toHaveBeenCalledTimes(2);
    stop();
    advance(DEDUPE_MS + 1);
    form.dispatchEvent(new Event("submit"));
    expect(onSubmit).toHaveBeenCalledTimes(2);
  });

  it("does nothing on pages without a login form", () => {
    const onSubmit = vi.fn();
    const doc = page(`<form><input name="q"></form>`);
    watchSubmissions(doc, onSubmit, undefined, () => true);
    doc.querySelector("form")!.dispatchEvent(new Event("submit"));
    expect(onSubmit).not.toHaveBeenCalled();
  });
});

describe("showSavePrompt", () => {
  it("renders in a closed shadow root and handles save", () => {
    page("");
    const onSave = vi.fn();
    const { root } = showSavePrompt(document, { status: "new", username: "me" }, { onSave, onDismiss: vi.fn() });
    const host = document.getElementById(PROMPT_HOST_ID)!;
    expect(host.shadowRoot).toBeNull(); // closed: invisible to the page
    expect(root.textContent).toContain("Save this login in Password365?");
    expect(root.textContent).toContain("me");
    root.querySelector<HTMLButtonElement>("button.primary")!.click();
    expect(onSave).toHaveBeenCalled();
    expect(document.getElementById(PROMPT_HOST_ID)).toBeNull();
  });

  it("offers update for changed passwords, dismisses, and replaces an old prompt", () => {
    page("");
    const onDismiss = vi.fn();
    showSavePrompt(document, { status: "new" }, { onSave: vi.fn(), onDismiss: vi.fn() });
    const { root } = showSavePrompt(document, { status: "password_changed" }, { onSave: vi.fn(), onDismiss });
    expect(document.querySelectorAll(`#${PROMPT_HOST_ID}`)).toHaveLength(1);
    expect(root.textContent).toContain("Update the password");
    const [, notNow] = root.querySelectorAll("button");
    notNow!.click();
    expect(onDismiss).toHaveBeenCalled();
  });
});

describe("fillNewPassword", () => {
  it("fills every password field of a sign-up form", async () => {
    const { fillNewPassword } = await import("./fill");
    const f = findLoginFields(page(`<form><input name="user"><input type="password"><input type="password"></form>`))!;
    expect(fillNewPassword(f, "S3cret!")).toBe(true);
    expect(f.passwords.map((p) => p.value)).toEqual(["S3cret!", "S3cret!"]);
    expect(fillNewPassword({ ...f, passwords: [] }, "x")).toBe(false);
  });
});

describe("fillSuggested", () => {
  it("fills the clicked password field and an empty confirmation after it", async () => {
    const { fillSuggested } = await import("./fill");
    page(`<form><input type="password" id="cur" value="old"><input type="password" id="new">
      <input type="password" id="confirm"><input type="password" id="other" value="x"></form>`);
    const target = document.getElementById("new") as HTMLInputElement;
    expect(fillSuggested(target, "N3w!")).toBe(true);
    const value = (id: string) => (document.getElementById(id) as HTMLInputElement).value;
    expect([value("cur"), value("new"), value("confirm"), value("other")]).toEqual(["old", "N3w!", "N3w!", "x"]);
  });

  it("fills only the clicked field when it is not a password field or has no form", async () => {
    const { fillSuggested } = await import("./fill");
    page(`<input id="t"><input type="password" id="p1"><input type="password" id="p2">`);
    const text = document.getElementById("t") as HTMLInputElement;
    expect(fillSuggested(text, "abc")).toBe(true);
    expect((document.getElementById("p1") as HTMLInputElement).value).toBe("");
    const p1 = document.getElementById("p1") as HTMLInputElement;
    fillSuggested(p1, "abc");
    expect((document.getElementById("p2") as HTMLInputElement).value).toBe("abc");
    p1.disabled = true;
    expect(fillSuggested(p1, "zzz")).toBe(false);
  });
});

describe("submissions a script fires", () => {
  it("are ignored (a page could test password guesses through the save prompt)", () => {
    const doc = page(`<form><input name="user" value="me"><input type="password" value="guess"><button>Go</button></form>`);
    const onSubmit = vi.fn();
    // No click or key press by the user just now (what browsers report to scripts' submits).
    Object.defineProperty(navigator, "userActivation", { value: { isActive: false }, configurable: true });
    const stop = watchSubmissions(doc, onSubmit);
    doc.querySelector("form")!.dispatchEvent(new Event("submit"));
    doc.querySelector("button")!.click();
    doc.querySelector("input[type=password]")!.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    expect(onSubmit).not.toHaveBeenCalled();
    stop();
    delete (navigator as { userActivation?: unknown }).userActivation;
  });
});

