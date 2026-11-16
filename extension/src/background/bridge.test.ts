import type { BridgeResponse } from "../shared/protocol";
import { Password365Error } from "../shared/protocol";
import { Bridge, type PortLike } from "./bridge";

function fakePort() {
  let onMessage: (m: BridgeResponse) => void = () => {};
  let onDisconnect: () => void = () => {};
  const sent: { id: number; type: string }[] = [];
  const port: PortLike = {
    postMessage: (m) => sent.push(m as { id: number; type: string }),
    onMessage: { addListener: (cb) => (onMessage = cb) },
    onDisconnect: { addListener: (cb) => (onDisconnect = cb) },
  };
  return { port, sent, reply: (m: BridgeResponse) => onMessage(m), disconnect: () => onDisconnect() };
}

describe("Bridge", () => {
  it("connects lazily, correlates ids and resolves results", async () => {
    const f = fakePort();
    const connect = vi.fn(() => f.port);
    const bridge = new Bridge(connect);
    expect(connect).not.toHaveBeenCalled();
    const a = bridge.request({ type: "status" });
    const b = bridge.request({ type: "logins_for_url", page_url: "https://a.com" });
    expect(f.sent).toEqual([
      { id: 1, type: "status" },
      { id: 2, type: "logins_for_url", page_url: "https://a.com" },
    ]);
    f.reply({ id: 2, ok: true, result: ["b"] });
    f.reply({ id: 1, ok: true, result: "a" });
    f.reply({ id: 99, ok: true, result: "ignored" });
    await expect(a).resolves.toBe("a");
    await expect(b).resolves.toEqual(["b"]);
    expect(connect).toHaveBeenCalledTimes(1);
  });

  it("rejects with the backend error code", async () => {
    const f = fakePort();
    const bridge = new Bridge(() => f.port);
    const p = bridge.request({ type: "status" });
    f.reply({ id: 1, ok: false, error: { code: "locked", message: "the vault is locked" } });
    await expect(p).rejects.toMatchObject({ code: "locked" });
    const q = bridge.request({ type: "status" });
    f.reply({ id: 2, ok: false });
    await expect(q).rejects.toMatchObject({ code: "internal" });
  });

  it("maps disconnects to app_not_running or host_missing and reconnects", async () => {
    const first = fakePort();
    const second = fakePort();
    const ports = [first.port, second.port];
    let lastError: string | undefined = "Native host has exited.";
    const bridge = new Bridge(() => ports.shift()!, () => lastError);

    const p = bridge.request({ type: "status" });
    first.disconnect();
    await expect(p).rejects.toMatchObject({ code: "app_not_running" });

    lastError = "Specified native messaging host not found.";
    const q = bridge.request({ type: "status" });
    second.disconnect();
    const err = (await q.catch((e: unknown) => e)) as Password365Error;
    expect(err).toBeInstanceOf(Password365Error);
    expect(err.code).toBe("host_missing");
  });

  it("times out", async () => {
    vi.useFakeTimers();
    const f = fakePort();
    const bridge = new Bridge(() => f.port, undefined, 100);
    const p = bridge.request({ type: "status" });
    vi.advanceTimersByTime(150);
    await expect(p).rejects.toMatchObject({ code: "timeout" });
    vi.useRealTimers();
  });
});
