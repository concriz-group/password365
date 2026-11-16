import { CLIPBOARD_CLEAR_MS, copySecret } from "./clipboard";
import { formatDate, initial, splitList } from "./format";

describe("copySecret", () => {
  function setup(readText?: () => Promise<string>) {
    const clipboard = { writeText: vi.fn().mockResolvedValue(undefined), readText };
    let scheduled: (() => void) | undefined;
    let delay = 0;
    const schedule = (fn: () => void, ms: number) => {
      scheduled = fn;
      delay = ms;
    };
    return { clipboard, schedule, fire: () => scheduled?.(), delay: () => delay };
  }

  it("copies and clears later if unchanged", async () => {
    const s = setup(() => Promise.resolve("pw"));
    await copySecret("pw", s.clipboard, s.schedule);
    expect(s.clipboard.writeText).toHaveBeenCalledWith("pw");
    expect(s.delay()).toBe(CLIPBOARD_CLEAR_MS);
    s.fire();
    await vi.waitFor(() => expect(s.clipboard.writeText).toHaveBeenLastCalledWith(""));
  });

  it("keeps copied text when clearing is off, and uses the chosen delay", async () => {
    const s = setup();
    await copySecret("pw", s.clipboard, s.schedule, 0);
    expect(s.delay()).toBe(0);
    await copySecret("pw", s.clipboard, s.schedule, 30_000);
    expect(s.delay()).toBe(30_000);
  });

  it("keeps the clipboard if the user copied something else", async () => {
    const s = setup(() => Promise.resolve("other"));
    await copySecret("pw", s.clipboard, s.schedule);
    s.fire();
    await new Promise((r) => setTimeout(r, 0));
    expect(s.clipboard.writeText).toHaveBeenCalledTimes(1);
  });

  it("clears when reading is not possible", async () => {
    const denied = setup(() => Promise.reject(new Error("denied")));
    await copySecret("pw", denied.clipboard, denied.schedule);
    denied.fire();
    await vi.waitFor(() => expect(denied.clipboard.writeText).toHaveBeenLastCalledWith(""));

    const noRead = setup(undefined);
    await copySecret("pw", noRead.clipboard, noRead.schedule);
    noRead.fire();
    await vi.waitFor(() => expect(noRead.clipboard.writeText).toHaveBeenLastCalledWith(""));
  });
});

describe("format helpers", () => {
  it("formats dates", () => {
    expect(formatDate(0, "en-US")).toMatch(/1970|1969/);
  });

  it("computes initials", () => {
    expect(initial(" github")).toBe("G");
    expect(initial("")).toBe("?");
  });

  it("splits lists", () => {
    expect(splitList("a, b\n\n c ,")).toEqual(["a", "b", "c"]);
  });
});
