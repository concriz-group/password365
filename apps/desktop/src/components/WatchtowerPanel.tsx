import { useEffect, useState } from "react";

import type { ItemRef, Watchtower } from "../api/types";
import { useClient } from "../api/context";
import { useT } from "../i18n";
import type { Translate } from "../../../../i18n/translate";
import { BreachCard } from "./BreachCard";
import { ErrorMessage } from "./ErrorMessage";

type Finding = "weak" | "reused" | "unsecured" | "expiring";

interface Card {
  key: Finding;
  title: string;
  text: string;
  items: { ref: ItemRef; note?: string }[];
}

function cards(w: Watchtower, t: Translate): Card[] {
  const card = (key: Finding, items: Card["items"]): Card => ({
    key,
    title: t(`watchtower.${key}Title`),
    text: t(`watchtower.${key}Text`),
    items,
  });
  return [
    card("weak", w.weak.map((ref) => ({ ref }))),
    card(
      "reused",
      w.reused.flatMap((group, i) => group.map((ref) => ({ ref, note: t("watchtower.group", { n: i + 1 }) }))),
    ),
    card("unsecured", w.unsecured.map((ref) => ({ ref }))),
    card(
      "expiring",
      w.expiring.map((e) => ({
        ref: e.item,
        note: t(e.expired ? "watchtower.expired" : "watchtower.expires", { date: e.date }),
      })),
    ),
  ];
}

/** The score as a ring, like 1Password's gauge. */
function Gauge({ score, rating }: { score: number; rating: string }) {
  const t = useT();
  const length = 2 * Math.PI * 52 * 0.75;
  const filled = (score / 1000) * length;
  return (
    <svg className="gauge" viewBox="0 0 120 120" width="150" height="150" role="img" aria-label={t("watchtower.score", { score, rating })}>
      <g transform="rotate(135 60 60)" fill="none" strokeWidth="10" strokeLinecap="round">
        <circle cx="60" cy="60" r="52" stroke="var(--border)" strokeDasharray={`${length} 1000`} />
        <circle cx="60" cy="60" r="52" className="gauge-fill" strokeDasharray={`${filled} 1000`} />
      </g>
      <text x="60" y="62" textAnchor="middle" className="gauge-score">
        {score}
      </text>
      <text x="60" y="80" textAnchor="middle" className="gauge-rating">
        {rating.toUpperCase()}
      </text>
    </svg>
  );
}

interface Props {
  onOpenItem: (id: string) => void;
  /** Changes when items may have changed (reload). */
  refreshKey?: number;
}

/** Watchtower: score, password strength and the problems found, all computed on this device. */
export function WatchtowerPanel({ onOpenItem, refreshKey = 0 }: Props) {
  const client = useClient();
  const t = useT();
  const [report, setReport] = useState<Watchtower | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [open, setOpen] = useState<Finding | null>(null);

  useEffect(() => {
    client.watchtower().then(setReport, setError);
  }, [client, refreshKey]);

  if (error) return <ErrorMessage error={error} />;
  if (!report) return <p className="muted">{t("watchtower.checking")}</p>;

  const found = cards(report, t).filter((c) => c.items.length > 0);
  const shown = found.find((c) => c.key === open);
  const total = report.strength.strong + report.strength.good + report.strength.weak;

  return (
    <section className="watchtower" aria-label={t("watchtower.title")}>
      <header className="watchtower-head">
        <div>
          <h1>{t("watchtower.title")}</h1>
          <p className="muted">{t("watchtower.intro")}</p>
        </div>
        {report.score !== null && report.rating && (
          <Gauge score={report.score} rating={t(`watchtower.rating_${report.rating}`)} />
        )}
      </header>
      {total > 0 && (
        <div className="panel-card">
          <h2>{t("watchtower.overall")}</h2>
          <div className="strength-bar" role="img" aria-label={t("watchtower.breakdown", report.strength)}>
            <span className="strong" style={{ flexGrow: report.strength.strong }} />
            <span className="good" style={{ flexGrow: report.strength.good }} />
            <span className="weak" style={{ flexGrow: report.strength.weak }} />
          </div>
        </div>
      )}
      {found.length === 0 ? (
        <p className="watchtower-clear">{t("watchtower.clear")}</p>
      ) : (
        <div className="finding-cards">
          {found.map((c) => (
            <div className="panel-card finding" key={c.key}>
              <strong className="finding-count">{c.items.length}</strong>
              <h2>{c.title}</h2>
              <p className="muted">{c.text}</p>
              <button type="button" className="link" onClick={() => setOpen(open === c.key ? null : c.key)}>
                {t(open === c.key ? "watchtower.hideItems" : "watchtower.showItems")}
              </button>
            </div>
          ))}
        </div>
      )}
      <BreachCard onOpenItem={onOpenItem} />
      {shown && (
        <ul className="finding-items" aria-label={shown.title}>
          {shown.items.map(({ ref, note }) => (
            <li key={`${note ?? ""}-${ref.id}`}>
              <button type="button" className="row" onClick={() => onOpenItem(ref.id)}>
                <span className="row-title">{ref.title}</span>
                {note && <span className="muted">{note}</span>}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
