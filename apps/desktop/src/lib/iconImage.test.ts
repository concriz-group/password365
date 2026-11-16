import { ICON_SIZE, toIcon } from "./iconImage";

describe("toIcon", () => {
  it("crops the picture to a centred square of the icon size", async () => {
    const drawImage = vi.fn();
    const canvas = { width: 0, height: 0, getContext: () => ({ drawImage }), toDataURL: () => "data:image/png;base64,AA==" };
    const doc = { createElement: () => canvas } as unknown as Document;
    vi.stubGlobal("createImageBitmap", vi.fn().mockResolvedValue({ width: 300, height: 100 }));
    expect(await toIcon(new Blob(), doc)).toBe("data:image/png;base64,AA==");
    expect(canvas.width).toBe(ICON_SIZE);
    expect(drawImage).toHaveBeenCalledWith(expect.anything(), 100, 0, 100, 100, 0, 0, ICON_SIZE, ICON_SIZE);
    const blind = { createElement: () => ({ ...canvas, getContext: () => null }) } as unknown as Document;
    await expect(toIcon(new Blob(), blind)).rejects.toThrow("no canvas");
    vi.unstubAllGlobals();
  });
});
