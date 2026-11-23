import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";

import { useClient } from "../api/context";
import { ErrorMessage } from "../components/ErrorMessage";
import { PasswordInput } from "../components/PasswordInput";
import { useAction } from "../hooks/useAction";
import { useT } from "../i18n";
import type { Invite } from "../api/types";
import { copySecret as copy } from "../lib/clipboard";

interface Props {
  onDone: () => void;
  onBack: () => void;
}

type Step = "show" | "code" | "verify" | "password";

/**
 * New install, set up from a device that already has Password365: paste the
 * pairing code shown there, compare the six digits, wait for approval, then
 * enter the master password.
 */
export function JoinDevice({ onDone, onBack }: Props) {
  const client = useClient();
  const t = useT();
  // First this computer shows a QR code for the device that has the vault to scan; the other way
  // (paste a code that device shows) is one click away.
  const [step, setStep] = useState<Step>("show");
  const [invite, setInvite] = useState<Invite | null>(null);
  const [code, setCode] = useState("");
  const [shown, setShown] = useState("");
  const [password, setPassword] = useState("");
  const [deviceName, setDeviceName] = useState("");

  const connect = useAction(async (e: FormEvent) => {
    e.preventDefault();
    setShown(await client.joinWithCode(code.trim()));
    setStep("verify");
  });
  const waitOnce = useCallback(async () => {
    await client.joinWait();
    setStep("password");
  }, [client]);
  const approval = useAction(waitOnce);
  // Wait for approval exactly once per pairing, however often this re-renders.
  const waiting = useRef(false);
  const finish = useAction(async (e: FormEvent) => {
    e.preventDefault();
    await client.joinComplete(password, deviceName);
    onDone();
  });

  // Step "show": start listening, show the QR code, and move on when a device connects.
  const showing = useRef(false);
  const [showError, setShowError] = useState<unknown>(null);
  useEffect(() => {
    if (step !== "show" || showing.current) return;
    showing.current = true;
    let live = true;
    client
      .joinShowCode()
      .then(async (shownInvite) => {
        if (!live) return;
        setInvite(shownInvite);
        const digits = await client.joinAcceptScan();
        if (!live) return;
        setShown(digits);
        setStep("verify");
      })
      .catch((reason) => {
        if (live) setShowError(reason);
        showing.current = false;
      });
    return () => {
      live = false;
    };
  }, [step, client]);

  const { run: waitForApproval } = approval;
  useEffect(() => {
    if (step === "verify" && !waiting.current) {
      waiting.current = true;
      void waitForApproval();
    }
  }, [step, waitForApproval]);

  function back() {
    void client.cancelPairing();
    onBack();
  }

  return (
    <main className="centered card">
      <h1>{t("join.title")}</h1>
      {step === "show" && (
        <>
          <p>
            {t("join.showIntro").split("{place}")[0]}
            <strong>{t("join.showPlace")}</strong>
            {t("join.showIntro").split("{place}")[1]}
          </p>
          {invite ? (
            <>
              <img className="qr" alt={t("join.showQr")} src={`data:image/svg+xml;utf8,${encodeURIComponent(invite.qr_svg)}`} />
              <p className="hint" role="status">
                {t("join.showWaiting")}
              </p>
              <button type="button" onClick={() => void copy(invite.code)}>
                {t("addDevice.copyCode")}
              </button>
            </>
          ) : showError ? (
            <ErrorMessage error={showError} />
          ) : (
            <p className="muted">{t("addDevice.preparing")}</p>
          )}
          <button
            type="button"
            className="link"
            onClick={() => {
              void client.cancelPairing();
              showing.current = false;
              setStep("code");
            }}
          >
            {t("join.pasteInstead")}
          </button>
        </>
      )}
      {step === "code" && (
        <form onSubmit={connect.run}>
          <p>
            {t("join.intro").split("{place}")[0]}
            <strong>{t("join.place")}</strong>
            {t("join.intro").split("{place}")[1]}
          </p>
          <label>
            {t("join.code")}
            <textarea value={code} onChange={(e) => setCode(e.target.value)} rows={3} autoFocus spellCheck={false} />
          </label>
          <ErrorMessage error={connect.error} />
          <button type="submit" className="primary" disabled={!code.trim() || connect.busy}>
            {connect.busy ? t("join.connecting") : t("common.continue")}
          </button>
        </form>
      )}
      {step === "verify" && (
        <>
          <p>{t("join.check")}</p>
          <p className="verification-code" aria-label={t("join.verification")}>
            {shown}
          </p>
          {approval.busy && (
            <p className="muted" role="status">
              {t("join.waiting")}
            </p>
          )}
          <ErrorMessage error={approval.error} />
        </>
      )}
      {step === "password" && (
        <form onSubmit={finish.run}>
          <p>{t("join.approved")}</p>
          <PasswordInput label={t("unlock.masterPassword")} value={password} onChange={setPassword} autoFocus />
          <label>
            {t("join.deviceName")}
            <input value={deviceName} onChange={(e) => setDeviceName(e.target.value)} placeholder={t("join.deviceNameExample")} />
          </label>
          <ErrorMessage error={finish.error} />
          <button type="submit" className="primary" disabled={!password || finish.busy}>
            {finish.busy ? t("join.settingUp") : t("join.finish")}
          </button>
        </form>
      )}
      <button type="button" className="link" onClick={back}>
        {t("common.back")}
      </button>
    </main>
  );
}
