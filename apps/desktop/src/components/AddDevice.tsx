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
  | { kind: "verify"; code: string }
  | { kind: "finishing" }
  | { kind: "failed"; error: ApiError };

/**
 * Add a device (as in 1Password): show a QR code / pairing code, compare the
 * six digits on both screens, choose vaults, then wait for the other device.
 */
export function AddDevice({ vaults, onAdded, onClose, copy }: Props) {
  const client = useClient();
  const t = useT();
  const [step, setStep] = useState<Step>({ kind: "opening" });
  const [chosen, setChosen] = useState<Set<string>>(() => new Set(vaults.map((v) => v.id)));
  const finished = useRef(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let alive = true;
    const fail = (e: unknown) => alive && setStep({ kind: "failed", error: toApiError(e) });
    client
      .inviteDevice()
      .then((invite) => {
        if (!alive) return null;
        setStep({ kind: "waiting", invite });
        return client.waitForNewDevice();
      })
      .then((code) => alive && code && setStep({ kind: "verify", code }), fail);
    return () => {
      alive = false;
      if (!finished.current) void client.cancelPairing();
    };
  }, [client]);

  async function approve() {
    setStep({ kind: "finishing" });
    try {
      const device = await client.approveNewDevice([...chosen]);
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
          <p>{t("addDevice.checkCode")}</p>
          <p className="verification-code" aria-label={t("addDevice.verificationCode")}>
            {step.code}
          </p>
          <fieldset>
            <legend>{t("addDevice.vaults")}</legend>
            {vaults.map((v) => (
              <label key={v.id} className="check">
                <input type="checkbox" checked={chosen.has(v.id)} onChange={() => toggle(v.id)} /> {v.name}
              </label>
            ))}
          </fieldset>
          <button type="button" className="primary" onClick={approve} disabled={chosen.size === 0}>
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
      {step.kind === "failed" && <ErrorMessage error={step.error} />}
      {step.kind !== "verify" && (
        <button type="button" className="ghost" onClick={onClose}>
          {step.kind === "failed" ? t("common.close") : t("common.cancel")}
        </button>
      )}
    </section>
  );
}
