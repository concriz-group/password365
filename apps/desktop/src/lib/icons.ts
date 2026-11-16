// The icon set in the repository's icons/ folder: 48×48 SVGs drawn with
// currentColor and white, each category's icon and colour, and the gallery
// people can pick from for an item.
import config from "../../../../icons/icons.json";

const files = import.meta.glob("../../../../icons/svg/*.svg", { eager: true, query: "?raw", import: "default" }) as Record<string, string>;

/** Icon name → SVG source (trusted: bundled from the repository, never from items). */
export const SVGS: Record<string, string> = Object.fromEntries(
  Object.entries(files).map(([path, svg]) => [/([a-z0-9_]+)\.svg$/.exec(path)![1]!, svg.trim()]),
);

/** Icons anyone can choose for an item. */
export const LIBRARY: string[] = config.library.filter((name) => name in SVGS);
/** Colours to choose from. */
export const COLORS: string[] = config.colors;
const FALLBACK_COLOR = COLORS[0] ?? "#2563eb";

/** A category's icon and colour. */
export function categoryIcon(kind: string): { name: string; color: string } | null {
  const entry = (config.categories as Record<string, { icon: string; color: string } | undefined>)[kind];
  return entry && entry.icon in SVGS ? { name: entry.icon, color: entry.color } : null;
}

/** What an item's stored icon is. */
export type IconValue = { type: "picture"; src: string } | { type: "builtin"; name: string; color: string };

/** Reads a stored icon: a picture (data URL) or `builtin:<name>:<colour>`; null for none or unknown ones. */
export function parseIcon(value: string | null | undefined): IconValue | null {
  if (!value) return null;
  if (value.startsWith("data:image/")) return { type: "picture", src: value };
  const match = /^builtin:([a-z0-9_]+)(?::(#[0-9a-fA-F]{6}))?$/.exec(value);
  if (!match || !(match[1]! in SVGS)) return null;
  return { type: "builtin", name: match[1]!, color: match[2] ?? FALLBACK_COLOR };
}

/** The stored value for a gallery icon. */
export const builtinIcon = (name: string, color: string) => `builtin:${name}:${color}`;
