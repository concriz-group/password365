import { useState, type FormEvent, type ReactNode } from "react";

import type { Density, OpenTo, Settings, Theme, Vault } from "../api/types";
import { useClient } from "../api/context";
import { useAction } from "../hooks/useAction";
import { usePasswordStrength } from "../hooks/usePasswordStrength";
import { useI18n } from "../i18n";
import { nativeFilePicker, type FilePicker } from "../lib/files";
import { useSettings } from "../settings";
import { BackupSection } from "./BackupSection";
import { DeveloperSettings } from "./DeveloperSettings";
import { PrivacySettings } from "./PrivacySettings";
import { EmergencyKitSection } from "./EmergencyKitSection";
import { EraseVault } from "./EraseVault";
import { ExportSection } from "./ExportSection";
import { ErrorMessage } from "./ErrorMessage";
import { PasswordInput } from "./PasswordInput";
import { CountrySelect } from "./CountrySelect";
import { ShortcutInput } from "./ShortcutInput";
import { ThirdPartyLicenses } from "./ThirdPartyLicenses";
import { UpdateSettings } from "./UpdateSettings";
import { HelpSection } from "./HelpSection";
import { StrengthMeter } from "./StrengthMeter";
import packageInfo from "../../package.json";

export const AUTO_LOCK_CHOICES = [1, 5, 10, 15, 30, 60, 120, 240];
export const CLIPBOARD_CHOICES = [0, 30, 90, 180, 300];
const ZOOM_STEP = 10;
const ZOOM_MIN = 50;
const ZOOM_MAX = 200;

const SECTIONS = ["general", "appearance", "security", "privacy", "backup", "export", "developer", "advanced", "about"] as const;
type Section = (typeof SECTIONS)[number];

interface Props {
  onClose: () => void;
  files?: FilePicker;
  vaults?: Vault[];
}

/** Settings like 1Password's: a list of sections on the left, the chosen one on the right. */
export function SettingsPanel({ onClose, files = nativeFilePicker, vaults = [] }: Props) {
  const { t } = useI18n();
  const { settings, loaded, update } = useSettings();
  const [section, setSection] = useState<Section>("general");
  const [saved, setSaved] = useState(false);

  const save = useAction(async (change: Partial<Settings>) => {
    setSaved(false);
    await update(change);
    setSaved(true);
  });
  const set = (change: Partial<Settings>) => void save.run(change);

  return (
    <section className="settings" aria-label={t("settings.title")}>
      <header className="toolbar-row">
        <h1>{t("settings.title")}</h1>
        <button type="button" className="ghost" onClick={onClose}>
          {t("common.close")}
        </button>
      </header>
      <div className="settings-layout">
        <nav className="settings-nav" aria-label={t("settings.sections")}>
          {SECTIONS.map((s) => (
            <button
              type="button"
              key={s}
              className={s === section ? "selected" : undefined}
              aria-current={s === section ? "page" : undefined}
              onClick={() => {
                setSection(s);
                setSaved(false);
              }}
            >
              {t(`settings.section_${s}`)}
            </button>
          ))}
        </nav>
        <div className="settings-body">
          {saved && (
            <p role="status" className="hint ok">
              {t("settings.saved")}
            </p>
          )}
          <ErrorMessage error={save.error} />
          {section === "general" && <General settings={settings} vaults={vaults} disabled={!loaded} set={set} />}
          {section === "appearance" && <Appearance settings={settings} disabled={!loaded} set={set} />}
          {section === "security" && <Security settings={settings} disabled={!loaded} set={set} />}
          {section === "privacy" && <PrivacySettings settings={settings} disabled={!loaded} set={set} />}
          {section === "backup" && <BackupSection files={files} />}
          {section === "export" && <ExportSection vaults={vaults} files={files} />}
          {section === "developer" && <DeveloperSettings settings={settings} disabled={!loaded} set={set} />}
          {section === "advanced" && <Advanced />}
          {section === "about" && <About />}
        </div>
      </div>
    </section>
  );
}

interface SectionProps {
  settings: Settings;
  disabled: boolean;
  set: (change: Partial<Settings>) => void;
}

function Check({ id, checked, disabled, onChange, children, hint }: { id: string; checked: boolean; disabled: boolean; onChange: (v: boolean) => void; children: ReactNode; hint?: string }) {
  return (
    <div className="field">
      <label className="checkbox" htmlFor={id}>
        <input id={id} type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
        {children}
      </label>
      {hint && <p className="hint">{hint}</p>}
    </div>
  );
}

function General({ settings, vaults, disabled, set }: SectionProps & { vaults: Vault[] }) {
  const { t } = useI18n();
  return (
    <>
      <h2>{t("settings.section_general")}</h2>
      <Check id="start-at-login" checked={settings.start_at_login} disabled={disabled} onChange={(v) => set({ start_at_login: v })}>
        {t("settings.startAtLogin")}
      </Check>
      <CountrySelect value={settings.region} disabled={disabled} onChange={(region) => set({ region })} />
      <h3>{t("settings.defaults")}</h3>
      <div className="field">
        <label htmlFor="default-vault">{t("settings.saveNewItemsIn")}</label>
        <select id="default-vault" value={settings.default_vault} disabled={disabled} onChange={(e) => set({ default_vault: e.target.value })}>
          <option value="">{t("settings.firstVault")}</option>
          {vaults.map((v) => (
            <option key={v.id} value={v.id}>
              {v.name}
            </option>
          ))}
        </select>
      </div>
      <div className="field">
        <label htmlFor="open-to">{t("settings.openTo")}</label>
        <select id="open-to" value={settings.open_to} disabled={disabled} onChange={(e) => set({ open_to: e.target.value as OpenTo })}>
          {(["all_items", "favorites", "watchtower"] as const).map((o) => (
            <option key={o} value={o}>
              {t(`settings.openTo_${o}`)}
            </option>
          ))}
        </select>
      </div>
      <h3>{t("shortcuts.title")}</h3>
      <ShortcutInput id="shortcut-quick" label={t("shortcuts.quickAccess")} value={settings.shortcut_quick_access} disabled={disabled} onChange={(v) => set({ shortcut_quick_access: v })} />
      <ShortcutInput id="shortcut-auto-type" label={t("shortcuts.autoType")} value={settings.shortcut_auto_type} disabled={disabled} onChange={(v) => set({ shortcut_auto_type: v })} />
      <ShortcutInput id="shortcut-lock" label={t("shortcuts.lock")} value={settings.shortcut_lock} disabled={disabled} onChange={(v) => set({ shortcut_lock: v })} />
    </>
  );
}

function Appearance({ settings, disabled, set }: SectionProps) {
  const { t } = useI18n();
  const zoom = (percent: number) => set({ zoom_percent: Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, percent)) });
  return (
    <>
      <h2>{t("settings.section_appearance")}</h2>
      <div className="field">
        <label htmlFor="theme">{t("settings.theme")}</label>
        <select id="theme" value={settings.theme} disabled={disabled} onChange={(e) => set({ theme: e.target.value as Theme })}>
          {(["system", "light", "dark"] as const).map((o) => (
            <option key={o} value={o}>
              {t(`settings.theme_${o}`)}
            </option>
          ))}
        </select>
      </div>
      <fieldset className="field density">
        <legend>{t("settings.density")}</legend>
        {(["comfortable", "compact"] as Density[]).map((d) => (
          <label key={d} className="checkbox">
            <input type="radio" name="density" value={d} checked={settings.density === d} disabled={disabled} onChange={() => set({ density: d })} />
            {t(`settings.density_${d}`)}
          </label>
        ))}
      </fieldset>
      <div className="field">
        <span className="label" id="zoom-label">
          {t("settings.zoom")}
        </span>
        <div className="field-row zoom" role="group" aria-labelledby="zoom-label">
          <button type="button" aria-label={t("settings.zoomOut")} disabled={disabled || settings.zoom_percent <= ZOOM_MIN} onClick={() => zoom(settings.zoom_percent - ZOOM_STEP)}>
            −
          </button>
          <output aria-live="polite">{settings.zoom_percent}%</output>
          <button type="button" aria-label={t("settings.zoomIn")} disabled={disabled || settings.zoom_percent >= ZOOM_MAX} onClick={() => zoom(settings.zoom_percent + ZOOM_STEP)}>
            +
          </button>
          <button type="button" className="ghost" disabled={disabled || settings.zoom_percent === 100} onClick={() => zoom(100)}>
            {t("settings.zoomReset")}
          </button>
        </div>
      </div>
      <h3>{t("settings.sidebar")}</h3>
      <Check id="sidebar-categories" checked={settings.sidebar_categories} disabled={disabled} onChange={(v) => set({ sidebar_categories: v })}>
        {t("settings.sidebarCategories")}
      </Check>
      <Check id="sidebar-tags" checked={settings.sidebar_tags} disabled={disabled} onChange={(v) => set({ sidebar_tags: v })}>
        {t("settings.sidebarTags")}
      </Check>
    </>
  );
}

function Security({ settings, disabled, set }: SectionProps) {
  const { t } = useI18n();
  return (
    <>
      <h2>{t("settings.section_security")}</h2>
      <h3>{t("settings.autoLockTitle")}</h3>
      <Check id="lock-on-sleep" checked={settings.lock_on_sleep} disabled={disabled} onChange={(v) => set({ lock_on_sleep: v })}>
        {t("settings.lockOnSleep")}
      </Check>
      <div className="field">
        <label htmlFor="auto-lock">{t("settings.autoLock")}</label>
        <select id="auto-lock" value={settings.auto_lock_minutes} disabled={disabled} onChange={(e) => set({ auto_lock_minutes: Number(e.target.value) })}>
          {AUTO_LOCK_CHOICES.map((m) => (
            <option key={m} value={m}>
              {m < 60 ? t("settings.minutes", { count: m }) : t("settings.hours", { count: m / 60 })}
            </option>
          ))}
        </select>
      </div>
      <h3>{t("settings.clipboard")}</h3>
      <div className="field">
        <label htmlFor="clipboard">{t("settings.clipboardClear")}</label>
        <select id="clipboard" value={settings.clipboard_clear_seconds} disabled={disabled} onChange={(e) => set({ clipboard_clear_seconds: Number(e.target.value) })}>
          {CLIPBOARD_CHOICES.map((s) => (
            <option key={s} value={s}>
              {s === 0 ? t("settings.clipboardNever") : s < 60 ? t("settings.seconds", { count: s }) : t("settings.minutes", { count: s / 60 })}
            </option>
          ))}
        </select>
      </div>
      <h3>{t("settings.concealed")}</h3>
      <Check id="wifi-qr" checked={settings.show_wifi_qr} disabled={disabled} onChange={(v) => set({ show_wifi_qr: v })}>
        {t("settings.showWifiQr")}
      </Check>
      <EmergencyKitSection />
      <ChangePassword />
    </>
  );
}

function ChangePassword() {
  const client = useClient();
  const { t } = useI18n();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [done, setDone] = useState(false);
  const nextStrength = usePasswordStrength(next);
  const change = useAction(async () => {
    await client.changeMasterPassword(current, next, confirm);
    setCurrent("");
    setNext("");
    setConfirm("");
    setDone(true);
  });
  function submit(e: FormEvent) {
    e.preventDefault();
    setDone(false);
    void change.run();
  }
  return (
    <>
      <h3>{t("settings.changePassword")}</h3>
      {done && (
        <p role="status" className="hint ok">
          {t("settings.passwordChanged")}
        </p>
      )}
      <form onSubmit={submit}>
        <PasswordInput label={t("settings.currentPassword")} value={current} onChange={setCurrent} />
        <PasswordInput label={t("settings.newPassword")} value={next} onChange={setNext} />
        {next && <StrengthMeter strength={nextStrength} />}
        <PasswordInput label={t("settings.repeatPassword")} value={confirm} onChange={setConfirm} />
        <ErrorMessage error={change.error} />
        <button type="submit" disabled={!current || !nextStrength?.acceptable || next !== confirm || change.busy}>
          {t("settings.changePassword")}
        </button>
      </form>
    </>
  );
}

function Advanced() {
  const { t, preference, languages, choose } = useI18n();
  const pick = useAction((tag: string) => choose(tag));
  const [erasing, setErasing] = useState(false);
  return (
    <>
      <h2>{t("settings.section_advanced")}</h2>
      <div className="field">
        <label htmlFor="language">{t("settings.language")}</label>
        <select id="language" value={preference} onChange={(e) => void pick.run(e.target.value)}>
          <option value="">{t("settings.systemLanguage")}</option>
          {languages.map((l) => (
            <option key={l.tag} value={l.tag} lang={l.tag}>
              {l.name}
            </option>
          ))}
        </select>
        <p className="hint">{t("settings.languageHint")}</p>
      </div>
      <ErrorMessage error={pick.error} />
      <h2>{t("erase.title")}</h2>
      {erasing ? (
        <EraseVault onCancel={() => setErasing(false)} />
      ) : (
        <>
          <p className="hint">{t("erase.settingsHint")}</p>
          <button type="button" className="danger" onClick={() => setErasing(true)}>
            {t("erase.title")}…
          </button>
        </>
      )}
    </>
  );
}

function About() {
  const { t } = useI18n();
  const [licences, setLicences] = useState(false);
  return (
    <>
      <h2>{t("settings.section_about")}</h2>
      <p>
        <strong>Password365</strong> {t("settings.version", { version: packageInfo.version })}
      </p>
      <p className="hint">{t("settings.aboutText")}</p>
      <UpdateSettings />
      <HelpSection />
      <p className="hint">{t("settings.license")}</p>
      <button type="button" className="link" aria-expanded={licences} onClick={() => setLicences(!licences)}>
        {t("settings.thirdParty")}
      </button>
      {licences && <ThirdPartyLicenses />}
    </>
  );
}
