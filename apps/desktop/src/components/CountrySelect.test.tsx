import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ClientProvider } from "../api/context";
import { fakeClient } from "../test/fakeClient";
import { CountrySelect } from "./CountrySelect";

const identity = { kind: "identity", label: "Identity", plural: "Identities", fields: [], regions: { BR: [], US: [] } };

describe("CountrySelect", () => {
  it("lists automatic, the countries with fields and the international choice", async () => {
    const onChange = vi.fn();
    const client = fakeClient({ categories: vi.fn().mockResolvedValue([identity]) });
    render(
      <ClientProvider client={client}>
        <CountrySelect value="" disabled={false} onChange={onChange} />
      </ClientProvider>,
    );
    const select = screen.getByLabelText("Country or region");
    expect(await screen.findByRole("option", { name: "Brazil" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "United States" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /^Automatic/ })).toHaveValue("");
    await userEvent.setup().selectOptions(select, "ZZ");
    expect(onChange).toHaveBeenCalledWith("ZZ");
  });
});
