import { useRef, useState } from "react";

import { useT } from "../i18n";
import { toIcon } from "../lib/iconImage";
import { COLORS, LIBRARY, builtinIcon, categoryIcon, parseIcon } from "../lib/icons";
import { Glyph, ItemIcon } from "./ItemIcon";

interface Props {
  title: string;
  /** Category (its icon is shown while the item has none of its own). */
  kind?: string;
  /** The icon shown (a data URL), or null for the default. */
  icon: string | null;
  onChange: (icon: string | null) => void;
  /** Turns a picked file into an icon (tests replace it: no canvas in jsdom). */
  convert?: (file: File) => Promise<string>;
}

/** The editor's item icon with its ▾ menu: Choose new icon…, Remove icon. */
export function IconPicker({ title, kind, icon, onChange, convert = toIcon }: Props) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [gallery, setGallery] = useState(false);
  const current = parseIcon(icon);
  const [color, setColor] = useState(current?.type === "builtin" ? current.color : (categoryIcon(kind ?? "")?.color ?? COLORS[0]!));
  const [failed, setFailed] = useState(false);
  const file = useRef<HTMLInputElement>(null);
  return (
    <div className="icon-picker">
      <ItemIcon title={title} icon={icon} kind={kind} large />
      <button
        type="button"
        className="icon-menu"
        aria-label={t("itemEditor.iconMenu")}
        aria-expanded={open || gallery}
        onClick={() => {
          setOpen((o) => !o);
          setGallery(false);
        }}
      >
        ▾
      </button>
      {open && (
        <div className="add-field-menu" role="menu">
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              setGallery(true);
            }}
          >
            {t("itemEditor.chooseFromGallery")}
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              file.current?.click();
            }}
          >
            {t("itemEditor.chooseIcon")}
          </button>
          {icon && (
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                setOpen(false);
                onChange(null);
              }}
            >
              {t("itemEditor.removeIcon")}
            </button>
          )}
        </div>
      )}
      {gallery && (
        <div className="add-field-menu icon-gallery" role="dialog" aria-label={t("itemEditor.gallery")} onKeyDown={(e) => e.key === "Escape" && setGallery(false)}>
          <div className="swatches" role="group" aria-label={t("itemEditor.galleryColor")}>
            {COLORS.map((c) => (
              <button key={c} type="button" className="swatch" style={{ background: c }} aria-label={c} aria-pressed={c === color} onClick={() => setColor(c)} />
            ))}
          </div>
          <div className="glyphs">
            {LIBRARY.map((name) => (
              <button
                key={name}
                type="button"
                aria-label={t(`icons.${name}`)}
                title={t(`icons.${name}`)}
                onClick={() => {
                  setGallery(false);
                  onChange(builtinIcon(name, color));
                }}
              >
                <Glyph name={name} color={color} />
              </button>
            ))}
          </div>
        </div>
      )}
      <input
        ref={file}
        type="file"
        accept="image/png,image/jpeg,image/webp,image/gif"
        hidden
        aria-label={t("itemEditor.chooseIcon")}
        onChange={(e) => {
          const picked = e.target.files?.[0];
          e.target.value = "";
          if (!picked) return;
          setFailed(false);
          convert(picked).then(onChange, () => setFailed(true));
        }}
      />
      {failed && <p className="hint warn">{t("itemEditor.iconFailed")}</p>}
    </div>
  );
}
