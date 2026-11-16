import { Fragment, type ReactNode } from "react";

/**
 * Splits a translated `text` on `{name}` placeholders and puts the matching
 * element of `parts` in their place, so translators can move links and
 * emphasis around: `rich(t("x.hint"), { page: <code>…</code> })`.
 */
export function rich(text: string, parts: Record<string, ReactNode>): ReactNode {
  return text.split(/(\{\w+\})/).map((piece, i) => {
    const name = /^\{(\w+)\}$/.exec(piece)?.[1];
    return <Fragment key={i}>{name && name in parts ? parts[name] : piece}</Fragment>;
  });
}
