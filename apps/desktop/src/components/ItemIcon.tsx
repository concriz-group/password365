import { initial } from "../lib/format";
import { SVGS, categoryIcon, parseIcon } from "../lib/icons";

interface Props {
  title: string;
  /** The item's own icon (a picture or a gallery icon), if any. */
  icon?: string | null;
  /** Category: its icon when the item has none of its own. */
  kind?: string;
  large?: boolean;
}

/** A coloured tile with a white glyph (two colours, like the app's own icon). */
export function Glyph({ name, color, large = false }: { name: string; color: string; large?: boolean }) {
  return (
    <span
      className={large ? "avatar large glyph" : "avatar glyph"}
      style={{ background: color }}
      aria-hidden="true"
      // Bundled SVG from the repository's icons/ folder, looked up by a checked name.
      dangerouslySetInnerHTML={{ __html: SVGS[name] ?? "" }}
    />
  );
}

/** An item's icon: its own picture or gallery icon, else its category's, else the title's first letter. */
export function ItemIcon({ title, icon, kind, large = false }: Props) {
  const own = parseIcon(icon);
  if (own?.type === "picture") {
    return <img className={large ? "avatar large picture" : "avatar picture"} src={own.src} alt="" aria-hidden="true" />;
  }
  const glyph = own ?? (kind ? categoryIcon(kind) : null);
  if (glyph) return <Glyph name={glyph.name} color={glyph.color} large={large} />;
  return (
    <span className={large ? "avatar large" : "avatar"} aria-hidden="true">
      {initial(title)}
    </span>
  );
}
