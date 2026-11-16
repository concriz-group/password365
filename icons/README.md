# Icons

48×48 SVG files in `svg/`, drawn with **currentColor** (the tile's white glyph)
and at most white details — two colours, like the app's own keyhole icon.

`icons.json` says:
- `categories`: each category's icon and colour (shown in lists, the sidebar,
  the new-item picker and item headers when an item has no icon of its own);
- `library`: the icons anyone can pick for an item (editor › icon ▾ › Choose from gallery…);
- `colors`: the colours offered with them.

Adding an icon: draw it in the same style (`viewBox="0 0 48 48"`, `stroke-width="4"`,
round caps and joins, no fill except white or currentColor details), save it as
`svg/<name>.svg`, list it in `icons.json`, and name it in `i18n/locales/*/icons.json`.
