import { useCallback, useState } from "react";

import { toApiError, type ApiError } from "../api/client";

/**
 * Wraps an async action with `busy` and `error` state so every form handles
 * submission the same way.
 */
export function useAction<A extends unknown[], R>(action: (...args: A) => Promise<R>) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);

  const run = useCallback(
    async (...args: A): Promise<R | undefined> => {
      setBusy(true);
      setError(null);
      try {
        return await action(...args);
      } catch (reason) {
        setError(toApiError(reason));
        return undefined;
      } finally {
        setBusy(false);
      }
    },
    [action],
  );

  return { run, busy, error, clearError: () => setError(null) };
}
