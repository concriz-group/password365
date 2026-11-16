import type { PasswordStrength } from "../api/types";

const LABELS = ["very_weak", "weak", "fair", "good", "strong"] as const;

/**
 * Length-only stand-in for the core's zxcvbn estimate, for the browser demo
 * and tests. Never used by the real app (the Rust core decides there).
 */
export function roughStrength(password: string): PasswordStrength {
  const length = [...password].length;
  const score = Math.min(4, Math.floor(length / 4));
  const missing = Math.max(0, 10 - length);
  return {
    score,
    label: LABELS[score]!,
    missing_chars: missing,
    warning: null,
    suggestions: [],
    acceptable: missing === 0 && score >= 3,
  };
}
