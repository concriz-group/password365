import { hiddenFromUser } from "./visible";

function field(html: string): HTMLInputElement {
  document.body.innerHTML = html;
  return document.querySelector("#f") as HTMLInputElement;
}

function laidOut(el: Element, left: number, top: number, width: number, height: number) {
  el.getClientRects = () => [{}] as unknown as DOMRectList;
  el.getBoundingClientRect = () => ({ left, top, width, height, right: left + width, bottom: top + height }) as DOMRect;
}

describe("hiddenFromUser", () => {
  it("accepts an ordinary field", () => {
    expect(hiddenFromUser(field(`<input id="f">`))).toBe(false);
    const f = field(`<input id="f">`);
    laidOut(f, 10, 10, 200, 30);
    expect(hiddenFromUser(f)).toBe(false);
  });

  it("refuses fields hidden by type, attribute or CSS on them or a parent", () => {
    expect(hiddenFromUser(field(`<input id="f" type="hidden">`))).toBe(true);
    expect(hiddenFromUser(field(`<div hidden><input id="f"></div>`))).toBe(true);
    expect(hiddenFromUser(field(`<div style="display:none"><input id="f"></div>`))).toBe(true);
    expect(hiddenFromUser(field(`<input id="f" style="visibility:hidden">`))).toBe(true);
    expect(hiddenFromUser(field(`<div style="opacity:0"><input id="f"></div>`))).toBe(true);
    expect(hiddenFromUser(field(`<input id="f" style="opacity:0.05">`))).toBe(true);
    expect(hiddenFromUser(field(`<input id="f" style="position:absolute;clip:rect(0,0,0,0)">`))).toBe(true);
  });

  it("refuses tiny and off-screen fields", () => {
    const tiny = field(`<input id="f">`);
    laidOut(tiny, 10, 10, 1, 1);
    expect(hiddenFromUser(tiny)).toBe(true);
    const away = field(`<input id="f">`);
    laidOut(away, -9999, 10, 200, 30);
    expect(hiddenFromUser(away)).toBe(true);
  });
});
