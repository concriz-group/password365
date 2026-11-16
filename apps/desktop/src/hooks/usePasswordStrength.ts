import { useEffect, useState } from "react";

import { useClient } from "../api/context";
import type { PasswordStrength } from "../api/types";

/** Wait this long after the last keystroke before asking the core. */
export const STRENGTH_DELAY_MS = 120;

/**
 * Live strength of `password`, estimated by the core. Returns `null` while the
 * field is empty or the estimate for the *current* value is not in yet, so a
 * stale "strong" result can never enable a button.
 */
export function usePasswordStrength(password: string): PasswordStrength | null {
  const client = useClient();
  const [result, setResult] = useState<{ password: string; strength: PasswordStrength } | null>(null);

  useEffect(() => {
    if (!password) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      client.passwordStrength(password).then(
        (strength) => !cancelled && setResult({ password, strength }),
        () => undefined, // the meter is advisory; the core validates on submit anyway
      );
    }, STRENGTH_DELAY_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [client, password]);

  return password && result?.password === password ? result.strength : null;
}
