/** Formats a millisecond timestamp as a short local date-time, in the interface language by default. */
export function formatDate(ms: number, locale?: string): string {
  const language = locale ?? (typeof document === "undefined" ? "" : document.documentElement.lang);
  return new Date(ms).toLocaleString(language || undefined, { dateStyle: "medium", timeStyle: "short" });
}

/** First letter of a title, for list avatars. */
export function initial(title: string): string {
  return (title.trim()[0] ?? "?").toUpperCase();
}

/** Splits a multi-line / comma separated text field into trimmed non-empty values. */
export function splitList(text: string): string[] {
  return text
    .split(/[\n,]/)
    .map((v) => v.trim())
    .filter((v) => v.length > 0);
}
