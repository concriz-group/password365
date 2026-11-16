import { useEffect, useMemo, useState } from "react";

import { useI18n } from "../i18n";

interface Package {
  name: string;
  version: string;
  license: string;
  ecosystem: "rust" | "javascript";
  texts: string[];
}
export interface LicenceData {
  packages: Package[];
  texts: Record<string, string>;
}

/** Loaded only when the screen opens (about 1 MB of licence texts). */
const load = (): Promise<LicenceData> =>
  import("../generated/third-party-licenses.json").then((m) => m.default as LicenceData);

/**
 * Settings › About › Third-party licenses, like 1Password's: every package
 * the app ships, its licence, and the licence text it came with.
 */
export function ThirdPartyLicenses({ source = load }: { source?: () => Promise<LicenceData> }) {
  const { t } = useI18n();
  const [data, setData] = useState<LicenceData | null>(null);
  const [query, setQuery] = useState("");
  useEffect(() => {
    void source().then(setData);
  }, [source]);
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (data?.packages ?? []).filter((p) => !q || p.name.toLowerCase().includes(q) || p.license.toLowerCase().includes(q));
  }, [data, query]);

  if (!data) return <p className="muted">{t("common.loading")}</p>;
  return (
    <div className="licences">
      <p className="hint">{t("settings.thirdPartyIntro", { count: data.packages.length })}</p>
      <input type="search" aria-label={t("common.search")} placeholder={t("common.search")} value={query} onChange={(e) => setQuery(e.target.value)} />
      <ul>
        {shown.map((p) => (
          <li key={`${p.ecosystem}:${p.name}@${p.version}`}>
            <details>
              <summary>
                <span className="licence-name">{p.name}</span> <span className="muted">{p.version}</span>
                <span className="licence-id">{p.license}</span>
              </summary>
              {p.texts.length === 0 ? (
                <p className="hint">{t("settings.noLicenceFile", { license: p.license })}</p>
              ) : (
                p.texts.map((id) => (
                  <pre key={id} className="licence-text">
                    {data.texts[id]}
                  </pre>
                ))
              )}
            </details>
          </li>
        ))}
      </ul>
    </div>
  );
}
