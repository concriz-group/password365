import { useEffect, useRef, useState } from "react";

import { useClient } from "../api/context";
import type { Device, Invite, Vault } from "../api/types";
import { toApiError, type ApiError } from "../api/client";
import { useT } from "../i18n";
import { rich } from "../i18n/richText";
import { ErrorMessage } from "./ErrorMessage";

interface Props {
  vaults: Vault[];
  onAdded: (device: Device) => void;
  onClose: () => void;
  /** Copies the pairing code (the clipboard is cleared again later). */
  copy?: (text: string) => Promise<void>;
}

type Step =
  | { kind: "opening" }
  | { kind: "waiting"; invite: Invite }
  | { kind: "verify" }
  | { kind: "finishing" }
  | { kind: "failed"; error: ApiError };

/**
 * Add a device: show a QR code / pairing code; once a device connects, the
 * user types the six digits it shows (typing, not just a click, makes sure the
 * device that connected is theirs), chooses vaults, then waits for it.
 */
export function AddDevice({ vaults, onAdded, onClose, copy }: Props) {
  const client = useClient();
  const t = useT();
  const [step, setStep] = useState<Step>({ kind: "opening" });
  const [chosen, setChosen] = useState<Set<string>>(() => new Set(vaults.map((v) => v.id)));
  const finished = useRef(false);
  const [copied, setCopied] = useState(false);
  const [typed, setTyped] = useState("");
  const digits = typed.replace(/\D/g, "");

  useEffect(() => {
    let alive = true;
    const fail = (e: unknown) => alive && setStep({ kind: "failed", error: toApiError(e) });
    client
      .inviteDevice()
      .then((invite) => {
        if (!alive) return null;
        setStep({ kind: "waiting", invite });
        return client.waitForNewDevice().then(() => true);
      })
      .then((connected) => alive && connected && setStep({ kind: "verify" }), fail);
    return () => {
      alive = false;
      if (!finished.current) void client.cancelPairing();
    };
  }, [client]);

  async function approve() {
    setStep({ kind: "finishing" });
    try {
      const device = await client.approveNewDevice(digits, [...chosen]);
      finished.current = true;
      onAdded(device);
    } catch (e) {
      setStep({ kind: "failed", error: toApiError(e) });
    }
  }

  function toggle(id: string) {
    const next = new Set(chosen);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setChosen(next);
  }

  return (
    <section className="add-device" aria-label={t("addDevice.title")}>
      <h2>{t("addDevice.title")}</h2>
      {step.kind === "opening" && <p className="muted">{t("addDevice.preparing")}</p>}
      {step.kind === "waiting" && (
        <>
          <p className="hint">{rich(t("addDevice.scanHint"), { setup: <strong>{t("addDevice.setupChoice")}</strong> })}</p>
          <img className="qr" alt={t("addDevice.qr")} src={`data:image/svg+xml;utf8,${encodeURIComponent(step.invite.qr_svg)}`} />
          <code className="pairing-code">{step.invite.code}</code>
          {copy && (
            <button
              type="button"
              onClick={() => {
                void copy(step.invite.code).then(() => setCopied(true));
              }}
            >
              {copied ? t("common.copied") : t("addDevice.copyCode")}
            </button>
          )}
          <p className="muted" role="status">
            {t("addDevice.waiting")}
          </p>
        </>
      )}
      {step.kind === "verify" && (
        <>
          <label>
            {t("addDevice.checkCode")}
            <input
              className="verification-code"
              aria-label={t("addDevice.verificationCode")}
              inputMode="numeric"
              autoComplete="off"
              autoFocus
              maxLength={7}
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
            />
          </label>
          <fieldset>
            <legend>{t("addDevice.vaults")}</legend>
            {vaults.map((v) => (
              <label key={v.id} className="check">
                <input type="checkbox" checked={chosen.has(v.id)} onChange={() => toggle(v.id)} /> {v.name}
              </label>
            ))}
          </fieldset>
          <button type="button" className="primary" onClick={approve} disabled={chosen.size === 0 || digits.length !== 6}>
            {t("addDevice.match")}
          </button>
          <button type="button" className="ghost" onClick={onClose}>
            {t("addDevice.noMatch")}
          </button>
        </>
      )}
      {step.kind === "finishing" && (
        <p className="muted" role="status">
          {t("addDevice.finishing")}
        </p>
      )}
      {step.kind === "failed" &&
        (step.error.message === "wrong_pairing_code" ? (
          <p role="alert" className="error">
            {t("addDevice.wrongCode")}
          </p>
        ) : (
          <ErrorMessage error={step.error} />
        ))}
      {step.kind !== "verify" && (
        <button type="button" className="ghost" onClick={onClose}>
          {step.kind === "failed" ? t("common.close") : t("common.cancel")}
        </button>
      )}
    </section>
  );
}
