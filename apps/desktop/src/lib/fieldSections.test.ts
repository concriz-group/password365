import { describe, expect, it } from "vitest";

import type { Category, ItemField } from "../api/types";
import { FIELD_TYPES, addField, asItemFields, moveBefore, moveBy, defaultRegion, systemRegion, duplicateLabel, freeLabel, grouped, initialFields, regionsOf, specOf, templateIn, toFieldValues } from "./fieldSections";

const identity: Category = {
  kind: "identity",
  label: "Identity",
  plural: "Identities",
  fields: [
    { label: "first name", input: "text", concealed: false, section: "Identification" },
    { label: "zip", input: "text", concealed: false, section: "Address" },
    { label: "email", input: "email", concealed: false, section: "Internet Details" },
  ],
};

const field = (label: string, section: string, value: string | null = "x"): ItemField => ({
  label,
  section,
  value,
  concealed: false,
  input: "text",
});

const type = (name: string) => FIELD_TYPES.find((t) => t.name === name)!;

describe("fieldSections", () => {
  it("puts the item's own fields at the end of their section", () => {
    const { rows, sections } = initialFields(identity, [
      field("first name", "Identification", "Ana"),
      field("gate code", "Address"),
      field("pet", "Family"),
    ]);
    expect(rows.map((r) => r.label)).toEqual(["first name", "zip", "gate code", "email", "pet"]);
    expect(rows[0]!).toMatchObject({ value: "Ana", custom: false });
    expect(rows[2]!).toMatchObject({ custom: true, sectionId: rows[1]!.sectionId });
    expect(sections.map((s) => [s.name, s.custom])).toEqual([
      ["Identification", false],
      ["Address", false],
      ["Internet Details", false],
      ["Family", true],
    ]);
  });

  it("gives an item without fields one unnamed section", () => {
    const { rows, sections } = initialFields(undefined, []);
    expect(rows).toEqual([]);
    expect(sections).toMatchObject([{ name: "", custom: false }]);
    expect(initialFields(undefined, [field("a", "", null)]).rows[0]!.value).toBe("");
  });

  it("adds a typed field at the end of a section with a free label", () => {
    const { rows: start, sections } = initialFields(identity, []);
    const first = sections[0]!.id;
    let rows = addField(addField(start, first, type("Phone")), first, type("Phone"));
    expect(rows.map((r) => r.label)).toEqual(["first name", "phone", "phone 2", "zip", "email"]);
    rows = addField(rows, "new", type("Password"));
    expect(rows.at(-1)).toMatchObject({ sectionId: "new", concealed: true, input: "text", label: "password" });
    expect(freeLabel(rows, "email")).toBe("email 2");
  });

  it("finds labels used twice among filled fields", () => {
    const rows = initialFields(identity, []).rows.map((r) => ({ ...r, value: "v" }));
    expect(duplicateLabel(rows)).toBeNull();
    rows[1]!.label = " First Name ";
    expect(duplicateLabel(rows)).toBe("First Name");
    rows[1]!.value = "";
    expect(duplicateLabel(rows)).toBeNull();
  });

  it("saves each row with its section's name and type", () => {
    const { rows, sections } = initialFields(identity, []);
    const family = { id: "s-family", name: " Family ", custom: true };
    const saved = toFieldValues(addField(rows, family.id, FIELD_TYPES[0]!), [...sections, family]);
    expect(saved.at(-1)).toEqual({ label: "text", value: "", section: "Family", input: "text", concealed: false });
    expect(toFieldValues(addField([], "gone", FIELD_TYPES[0]!), [])[0]!.section).toBe("");
  });

  it("groups in the given order, keeping empty groups", () => {
    const items = [{ s: "b" }, { s: "" }, { s: "b" }];
    expect(grouped(items, (i) => i.s, ["", "b", "c"]).map(([n, i]) => [n, i.length])).toEqual([
      ["", 1],
      ["b", 2],
      ["c", 0],
    ]);
  });
});

describe("countries", () => {
  const withBrazil = { ...identity, regions: { BR: [{ label: "CPF", input: "text" as const, concealed: false, section: "" }] } };

  it("uses the chosen country, else the system's region, never the interface language", () => {
    expect(defaultRegion(withBrazil, "BR", ["en-US"])).toBe("BR");
    expect(defaultRegion(withBrazil, "", ["en-BR"])).toBe("BR"); // English, living in Brazil
    expect(defaultRegion(withBrazil, "", ["pt-PT"])).toBe("");
    expect(defaultRegion(withBrazil, "ZZ", ["pt-BR"])).toBe(""); // international fields chosen
    expect(defaultRegion(identity, "BR", [])).toBe("");
    expect(systemRegion(["pt", "not a tag!", "es-AR"])).toBe("AR");
    expect(systemRegion([])).toBe("");
    expect(regionsOf(undefined)).toEqual([]);
  });

  it("finds a country's fields and carries values over", () => {
    expect(templateIn(withBrazil, "BR").map((f) => f.label)).toEqual(["CPF"]);
    expect(templateIn(withBrazil, "AR")).toBe(identity.fields);
    expect(templateIn(undefined, "")).toEqual([]);
    expect(specOf(withBrazil, "BR", "cpf")?.label).toBe("CPF");
    const { rows, sections } = initialFields(identity, []);
    rows[0]!.value = "Ana";
    expect(asItemFields(rows, sections)).toEqual([{ label: "first name", value: "Ana", concealed: false, input: "text", section: "Identification" }]);
    expect(asItemFields([{ ...rows[0]!, sectionId: "gone" }], [])[0]!.section).toBe("");
  });
});

describe("moving fields", () => {
  const { rows } = initialFields(identity, []);
  const labels = (all: typeof rows) => all.map((r) => r.label);

  it("moves before another field, taking its section", () => {
    const moved = moveBefore(rows, rows[2]!.id, rows[0]!.id);
    expect(labels(moved)).toEqual(["email", "first name", "zip"]);
    expect(moved[0]!.sectionId).toBe(rows[0]!.sectionId);
    expect(moveBefore(rows, rows[0]!.id, rows[0]!.id)).toBe(rows);
    expect(moveBefore(rows, "nope", rows[0]!.id)).toBe(rows);
  });

  it("moves one place, crossing into the next section at its edge", () => {
    const same = addField(rows, rows[0]!.sectionId, FIELD_TYPES[0]!);
    expect(labels(moveBy(same, same[1]!.id, -1))).toEqual(["text", "first name", "zip", "email"]);
    const crossed = moveBy(rows, rows[0]!.id, 1);
    expect(labels(crossed)).toEqual(labels(rows));
    expect(crossed[0]!.sectionId).toBe(rows[1]!.sectionId);
    expect(moveBy(rows, rows[0]!.id, -1)).toBe(rows);
  });
});
