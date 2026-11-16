import { useCallback, useRef, useState } from "react";

import { useT } from "../i18n";

export const LIST_WIDTH = { min: 240, max: 560, initial: 300 };
const KEY = "password365.listWidth";

export const clampWidth = (w: number) => Math.round(Math.min(LIST_WIDTH.max, Math.max(LIST_WIDTH.min, w)));

/** The item list's width, remembered on this computer. */
export function useListWidth(): [number, (w: number) => void] {
  const [width, setWidth] = useState(() => {
    try {
      const saved = Number(localStorage.getItem(KEY));
      return saved ? clampWidth(saved) : LIST_WIDTH.initial;
    } catch {
      return LIST_WIDTH.initial;
    }
  });
  const set = useCallback((w: number) => {
    const next = clampWidth(w);
    setWidth(next);
    try {
      localStorage.setItem(KEY, String(next));
    } catch {
      // Not remembered: fine.
    }
  }, []);
  return [width, set];
}

/**
 * The line between the item list and the item: drag it (or use the arrow
 * keys) to make the list wider; double-click to reset.
 */
export function PaneResizer({ width, onChange }: { width: number; onChange: (w: number) => void }) {
  const t = useT();
  const start = useRef<{ x: number; width: number } | null>(null);
  return (
    <div
      className="pane-resizer"
      role="separator"
      aria-orientation="vertical"
      aria-label={t("list.resize")}
      aria-valuemin={LIST_WIDTH.min}
      aria-valuemax={LIST_WIDTH.max}
      aria-valuenow={width}
      tabIndex={0}
      onPointerDown={(e) => {
        start.current = { x: e.clientX, width };
        e.currentTarget.setPointerCapture?.(e.pointerId);
      }}
      onPointerMove={(e) => {
        if (start.current) onChange(start.current.width + e.clientX - start.current.x);
      }}
      onPointerUp={() => (start.current = null)}
      onDoubleClick={() => onChange(LIST_WIDTH.initial)}
      onKeyDown={(e) => {
        if (e.key === "ArrowLeft") onChange(width - 16);
        else if (e.key === "ArrowRight") onChange(width + 16);
        else return;
        e.preventDefault();
      }}
    />
  );
}
