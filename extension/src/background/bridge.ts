import { type BridgeBody, type BridgeResponse, Password365Error } from "../shared/protocol";

/** The subset of `chrome.runtime.Port` the bridge uses (injectable for tests). */
export interface PortLike {
  postMessage(message: unknown): void;
  onMessage: { addListener(cb: (message: BridgeResponse) => void): void };
  onDisconnect: { addListener(cb: () => void): void };
}

interface Pending {
  resolve: (value: unknown) => void;
  reject: (error: Password365Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

export const REQUEST_TIMEOUT_MS = 15_000;

/**
 * Request/response channel to the native host. Connects lazily, correlates
 * responses by id and turns host failures into `Password365Error`s.
 */
export class Bridge {
  private port: PortLike | null = null;
  private nextId = 1;
  private readonly pending = new Map<number, Pending>();

  constructor(
    private readonly connect: () => PortLike,
    private readonly lastError: () => string | undefined = () => undefined,
    private readonly timeoutMs = REQUEST_TIMEOUT_MS,
  ) {}

  request<T>(body: BridgeBody): Promise<T> {
    const port = this.ensurePort();
    const id = this.nextId++;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Password365Error("timeout", "Password365 did not answer in time"));
      }, this.timeoutMs);
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject, timer });
      port.postMessage({ id, ...body });
    });
  }

  private ensurePort(): PortLike {
    if (this.port) return this.port;
    const port = this.connect();
    port.onMessage.addListener((message) => this.settle(message));
    port.onDisconnect.addListener(() => this.disconnected());
    this.port = port;
    return port;
  }

  private settle(message: BridgeResponse) {
    const pending = this.pending.get(message.id);
    if (!pending) return;
    this.pending.delete(message.id);
    clearTimeout(pending.timer);
    if (message.ok) pending.resolve(message.result);
    else pending.reject(new Password365Error(message.error?.code ?? "internal", message.error?.message ?? "error"));
  }

  private disconnected() {
    this.port = null;
    const reason = this.lastError() ?? "";
    const error = /not found|not installed|forbidden/i.test(reason)
      ? new Password365Error("host_missing", "The Password365 browser helper is not installed")
      : new Password365Error("app_not_running", "Password365 is not running — open the app and unlock it");
    for (const { reject, timer } of this.pending.values()) {
      clearTimeout(timer);
      reject(error);
    }
    this.pending.clear();
  }
}
