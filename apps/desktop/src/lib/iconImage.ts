// "Choose new icon…": the picked picture, scaled to a small square PNG so it
// syncs cheaply (the core refuses anything over 96 KB).

/** Edge of the stored icon, in pixels. */
export const ICON_SIZE = 64;

/** Draws `file` centred and cropped into an ICON_SIZE square; returns a PNG data URL. */
export async function toIcon(file: Blob, doc: Document = document): Promise<string> {
  const image = await createImageBitmap(file);
  const canvas = doc.createElement("canvas");
  canvas.width = ICON_SIZE;
  canvas.height = ICON_SIZE;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("no canvas");
  const side = Math.min(image.width, image.height);
  context.drawImage(image, (image.width - side) / 2, (image.height - side) / 2, side, side, 0, 0, ICON_SIZE, ICON_SIZE);
  return canvas.toDataURL("image/png");
}
