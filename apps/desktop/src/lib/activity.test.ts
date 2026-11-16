import { REPORT_EVERY_MS, watchActivity } from "./activity";

/** Events jsdom dispatches are untrusted; these stand for real input unless `bubbles` says otherwise. */
const byUser = (e: Event) => !e.bubbles;
function input(name: string, trusted = true) {
  window.dispatchEvent(new Event(name, { bubbles: !trusted }));
}

describe("watchActivity", () => {
  it("ignores events scripts dispatch by default", () => {
    const report = vi.fn();
    const stop = watchActivity(window, report);
    window.dispatchEvent(new Event("keydown"));
    expect(report).not.toHaveBeenCalled();
    stop();
  });

  it("reports real input, at most every few seconds, and stops when asked", () => {
    let time = 0;
    const report = vi.fn();
    const stop = watchActivity(window, report, () => time, byUser);
    input("keydown");
    input("pointermove");
    expect(report).toHaveBeenCalledTimes(1);
    time += REPORT_EVERY_MS;
    input("wheel");
    expect(report).toHaveBeenCalledTimes(2);
    time += REPORT_EVERY_MS;
    input("pointerdown", false); // a script's event is not the user
    expect(report).toHaveBeenCalledTimes(2);
    stop();
    input("keydown");
    expect(report).toHaveBeenCalledTimes(2);
  });
});
