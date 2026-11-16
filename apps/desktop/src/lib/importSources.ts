// The sources on the Migrate your data screen (templates/imports.json).
import imports from "../../../../templates/imports.json";

export type SourceGroup = "browser" | "manager" | "other";

export interface ImportSource {
  id: string;
  /** Product name (not translated); generic sources are named in the catalog. */
  name?: string;
  group: SourceGroup;
}

export const IMPORT_SOURCES = imports.sources as ImportSource[];
export const SOURCE_GROUPS: SourceGroup[] = ["browser", "manager", "other"];

/** Sources whose shown name contains `query` (case and accents ignored). */
export function matchingSources(sources: ImportSource[], query: string, nameOf: (s: ImportSource) => string): ImportSource[] {
  const plain = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const wanted = plain(query.trim());
  return wanted ? sources.filter((s) => plain(nameOf(s)).includes(wanted)) : sources;
}
