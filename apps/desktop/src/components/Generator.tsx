import { useCallback, useEffect, useState } from "react";

import type { CharClass, GenerateRequest, Generated } from "../api/types";
import { useClient } from "../api/context";
import { ALL_CLASSES, DEFAULT_PASSPHRASE, DEFAULT_PASSWORD, DEFAULT_PIN } from "../lib/generatorPresets";
import { useT } from "../i18n";
import { translateOr } from "../../../../i18n/translate";
import { ErrorMessage } from "./ErrorMessage";

type Mode = GenerateRequest["type"];

const PRESETS: Record<Mode, GenerateRequest> = {
  password: DEFAULT_PASSWORD,
  passphrase: DEFAULT_PASSPHRASE,
  pin: DEFAULT_PIN,
};

interface Props {
  copy: (text: string) => Promise<void>;
  onClose: () => void;
}

/** Password / passphrase / PIN generator with live preview. */
export function Generator({ copy, onClose }: Props) {
  const client = useClient();
  const t = useT();
  const [request, setRequest] = useState<GenerateRequest>(DEFAULT_PASSWORD);
  const [result, setResult] = useState<Generated | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [copied, setCopied] = useState(false);

  const regenerate = useCallback(() => {
    setCopied(false);
    client.generate(request).then(
      (r) => {
        setResult(r);
        setError(null);
      },
      setError,
    );
  }, [client, request]);

  useEffect(regenerate, [regenerate]);

  function toggleClass(cls: CharClass) {
    if (request.type !== "password") return;
    const classes = request.classes.includes(cls)
      ? request.classes.filter((c) => c !== cls)
      : ALL_CLASSES.filter((c) => c === cls || request.classes.includes(c));
    setRequest({ ...request, classes });
  }

  async function copyResult() {
    if (!result) return;
    await copy(result.value);
    setCopied(true);
  }

  return (
    <section className="generator" aria-label={t("generator.aria")}>
      <header className="toolbar-row">
        <h1>{t("generator.title")}</h1>
        <button type="button" className="ghost" onClick={onClose}>
          {t("common.close")}
        </button>
      </header>
      <div className="segmented" role="radiogroup" aria-label={t("generator.type")}>
        {(Object.keys(PRESETS) as Mode[]).map((mode) => (
          <button
            key={mode}
            type="button"
            role="radio"
            aria-checked={request.type === mode}
            className={request.type === mode ? "active" : ""}
            onClick={() => setRequest(PRESETS[mode])}
          >
            {t(`generator.mode_${mode}`)}
          </button>
        ))}
      </div>

      <output className="generated" aria-label={t("generator.value")}>
        {result?.value ?? ""}
      </output>
      {result && (
        <p className="muted">
          {t("generator.bits", {
            bits: Math.round(result.bits),
            strength: translateOr(t, `generator.strength_${result.strength}`, result.strength),
          })}
        </p>
      )}
      <ErrorMessage error={error} />

      {request.type === "password" && (
        <>
          <Range label={t("generator.length")} min={8} max={64} value={request.length} onChange={(length) => setRequest({ ...request, length })} />
          {ALL_CLASSES.map((cls) => (
            <label key={cls} className="checkbox">
              <input type="checkbox" checked={request.classes.includes(cls)} onChange={() => toggleClass(cls)} />
              {t(`generator.class_${cls}`)}
            </label>
          ))}
          <label className="checkbox">
            <input
              type="checkbox"
              checked={request.exclude_ambiguous}
              onChange={(e) => setRequest({ ...request, exclude_ambiguous: e.target.checked })}
            />
            {t("generator.ambiguous")}
          </label>
        </>
      )}
      {request.type === "passphrase" && (
        <>
          <Range label={t("generator.words")} min={3} max={12} value={request.words} onChange={(words) => setRequest({ ...request, words })} />
          <label className="checkbox">
            <input
              type="checkbox"
              checked={request.include_number}
              onChange={(e) => setRequest({ ...request, include_number: e.target.checked })}
            />
            {t("generator.includeNumber")}
          </label>
        </>
      )}
      {request.type === "pin" && (
        <Range label={t("generator.digits")} min={4} max={12} value={request.digits} onChange={(digits) => setRequest({ ...request, digits })} />
      )}

      <footer className="detail-footer">
        <button type="button" onClick={regenerate}>
          {t("generator.regenerate")}
        </button>
        <button type="button" onClick={copyResult} disabled={!result}>
          {t(copied ? "common.copied" : "common.copy")}
        </button>
      </footer>
    </section>
  );
}

function Range(props: { label: string; min: number; max: number; value: number; onChange: (v: number) => void }) {
  return (
    <label className="range">
      {props.label}: {props.value}
      <input
        type="range"
        min={props.min}
        max={props.max}
        value={props.value}
        onChange={(e) => props.onChange(Number(e.target.value))}
      />
    </label>
  );
}
