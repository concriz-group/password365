import { render, screen } from "@testing-library/react";

import type { PasswordStrength } from "../api/types";
import { StrengthMeter } from "./StrengthMeter";

const base: PasswordStrength = {
  score: 1,
  label: "weak",
  missing_chars: 0,
  warning: null,
  suggestions: [],
  acceptable: false,
};

describe("StrengthMeter", () => {
  it("prefers the core's warning, then its first suggestion", () => {
    const { rerender } = render(<StrengthMeter strength={{ ...base, warning: "This is a very common password." }} />);
    expect(screen.getByText(/very common password/)).toBeInTheDocument();
    rerender(<StrengthMeter strength={{ ...base, suggestions: ["Add another word or two."] }} />);
    expect(screen.getByText(/Add another word/)).toBeInTheDocument();
  });

  it("says one character in the singular and colours by result", () => {
    const { container } = render(<StrengthMeter strength={{ ...base, missing_chars: 1 }} />);
    expect(screen.getByText(/1 more character needed/)).toBeInTheDocument();
    expect(container.firstChild).toHaveClass("strength-weak");
    expect(container.querySelectorAll(".filled")).toHaveLength(2);
  });
});
