import { render } from "@testing-library/react";

import { rich } from "./richText";

describe("rich", () => {
  it("puts elements in place of their placeholders and leaves others as text", () => {
    const { container } = render(<p>{rich("Open {page} then {missing}.", { page: <code>x</code> })}</p>);
    expect(container.innerHTML).toBe("<p>Open <code>x</code> then {missing}.</p>");
  });
});
