import "@fontsource/nunito/latin-900.css";

import onLight from "../../../../branding/password365-logo.svg";
import onDark from "../../../../branding/password365-logo-dark.svg";

/** The 3·6·5 padlock, in the artwork for the current theme. */
export function BrandMark({ className = "" }: { className?: string }) {
  return (
    <>
      <img className={`brand-mark on-light ${className}`} src={onLight} alt="365" />
      <img className={`brand-mark on-dark ${className}`} src={onDark} alt="" aria-hidden="true" />
    </>
  );
}

/**
 * The name with the mark. `inline`: "Password" followed by the padlock,
 * which reads as 365 (the app's bar). `stacked`: the padlock above
 * "Password365" (welcome and unlock screens).
 */
export function BrandLockup({ layout }: { layout: "inline" | "stacked" }) {
  if (layout === "inline") {
    return (
      <span className="brand-inline" role="img" aria-label="Password365">
        <span className="brand-word" aria-hidden="true">
          password
        </span>
        <BrandMark className="brand-inline-mark" />
      </span>
    );
  }
  return (
    <div className="brand-stacked">
      <BrandMark />
      <span className="brand-word">
        password<span className="brand-365">365</span>
      </span>
    </div>
  );
}
