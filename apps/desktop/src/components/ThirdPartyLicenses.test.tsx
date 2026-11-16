import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { fakeClient, renderWithClient } from "../test/fakeClient";
import { ThirdPartyLicenses } from "./ThirdPartyLicenses";

const data = {
  packages: [
    { name: "serde", version: "1.0.0", license: "MIT OR Apache-2.0", ecosystem: "rust" as const, texts: ["a"] },
    { name: "react", version: "18.3.1", license: "MIT", ecosystem: "javascript" as const, texts: [] },
    { name: "uniffi", version: "0.32.2", license: "MPL-2.0", ecosystem: "rust" as const, texts: ["m"], standard: true },
  ],
  texts: { a: "Permission is hereby granted, free of charge…", m: "Mozilla Public License Version 2.0" },
};

describe("ThirdPartyLicenses", () => {
  it("lists packages with their licence and text, and filters", async () => {
    renderWithClient(<ThirdPartyLicenses source={() => Promise.resolve(data)} />, fakeClient());
    expect(await screen.findByText(/includes 3 open-source packages/)).toBeInTheDocument();
    expect(screen.getByText("MIT OR Apache-2.0")).toBeInTheDocument();
    expect(screen.getByText(/Permission is hereby granted/)).toBeInTheDocument();
    expect(screen.getByText("This package ships no licence file; its licence is MIT.")).toBeInTheDocument();
    expect(
      screen.getByText("This package ships no licence file; below is the standard text of its licence (MPL-2.0)."),
    ).toBeInTheDocument();
    expect(screen.getByText("Mozilla Public License Version 2.0")).toBeInTheDocument();
    await userEvent.type(screen.getByRole("searchbox"), "react");
    expect(screen.queryByText("serde")).not.toBeInTheDocument();
    expect(screen.getByText("react")).toBeInTheDocument();
  });

  it("loads the real list from the app", async () => {
    renderWithClient(<ThirdPartyLicenses />, fakeClient());
    expect(await screen.findByText(/open-source packages/, {}, { timeout: 5000 })).toBeInTheDocument();
    expect(screen.getByText("tauri")).toBeInTheDocument();
  });
});
