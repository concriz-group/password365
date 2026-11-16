import { relativeTime } from "./relativeTime";

describe("relativeTime", () => {
  const now = 10 * 24 * 60 * 60_000;
  it.each([
    [null, "never"],
    [now - 5_000, "just now"],
    [now + 5_000, "just now"],
    [now - 60_000, "1 minute ago"],
    [now - 5 * 60_000, "5 minutes ago"],
    [now - 3 * 60 * 60_000, "3 hours ago"],
    [now - 2 * 24 * 60 * 60_000, "2 days ago"],
  ])("%s → %s", (at, text) => {
    expect(relativeTime(at, now)).toBe(text);
  });
});
