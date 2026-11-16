import { useState } from "react";

import type { AttachmentInfo } from "../api/types";
import { useClient } from "../api/context";
import { useAction } from "../hooks/useAction";
import { useI18n } from "../i18n";
import type { FilePicker } from "../lib/files";
import { formatSize } from "../lib/size";
import { ErrorMessage } from "./ErrorMessage";

interface Props {
  itemId: string;
  attachments: AttachmentInfo[];
  files: FilePicker;
  /** The item changed (a file was added or removed): reload it. */
  onChanged: () => void;
}

/** The item's files: open, save a copy, remove, attach another. */
export function AttachmentsSection({ itemId, attachments, files, onChanged }: Props) {
  const client = useClient();
  const { t, language } = useI18n();
  const [removing, setRemoving] = useState<string | null>(null);
  const run = useAction(async (action: () => Promise<unknown>) => {
    await action();
  });

  const attach = () =>
    void run.run(async () => {
      const path = await files.openAnyFile();
      if (!path) return;
      await client.attachFile(itemId, path);
      onChanged();
    });
  const save = (a: AttachmentInfo) =>
    void run.run(async () => {
      const path = await files.saveFile(a.name);
      if (path) await client.saveAttachment(itemId, a.id, path);
    });
  const remove = (a: AttachmentInfo) =>
    void run.run(async () => {
      await client.removeAttachment(itemId, a.id);
      setRemoving(null);
      onChanged();
    });

  return (
    <section className="attachments" aria-label={t("attachments.title")}>
      {attachments.length > 0 && <h3 className="section-title">{t("attachments.title")}</h3>}
      <ul>
        {attachments.map((a) => (
          <li key={a.id} className="attachment">
            <span className="attachment-name">
              <strong>{a.name}</strong>
              <span className="muted">
                {" "}
                {formatSize(a.size, language)}
                {!a.available && ` · ${t("attachments.notHereYet")}`}
              </span>
            </span>
            <span className="detail-actions">
              <button type="button" className="ghost" disabled={!a.available} onClick={() => void run.run(() => client.openAttachment(itemId, a.id))}>
                {t("attachments.open")}
              </button>
              <button type="button" className="ghost" disabled={!a.available} onClick={() => save(a)}>
                {t("attachments.save")}
              </button>
              {removing === a.id ? (
                <>
                  <button type="button" className="danger" onClick={() => remove(a)}>
                    {t("attachments.confirmRemove")}
                  </button>
                  <button type="button" className="ghost" onClick={() => setRemoving(null)}>
                    {t("common.cancel")}
                  </button>
                </>
              ) : (
                <button type="button" className="ghost" aria-label={t("attachments.remove", { name: a.name })} onClick={() => setRemoving(a.id)}>
                  {t("common.remove")}
                </button>
              )}
            </span>
          </li>
        ))}
      </ul>
      <button type="button" className="link" onClick={attach} disabled={run.busy}>
        {t("attachments.attach")}
      </button>
      <ErrorMessage error={run.error} />
    </section>
  );
}
