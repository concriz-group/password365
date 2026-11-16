import type { SshApproval } from "../api/types";
import { useClient } from "../api/context";
import { useT } from "../i18n";

/** "Allow a program to use the SSH key …?" — once per key per unlock. */
export function SshApprovalPrompt({ request, onDone }: { request: SshApproval; onDone: () => void }) {
  const client = useClient();
  const t = useT();
  const answer = (allow: boolean) => {
    void client.sshAnswer(request.id, allow).finally(onDone);
  };
  return (
    <div className="modal-backdrop">
      <div className="modal" role="alertdialog" aria-labelledby="ssh-approve-title" aria-describedby="ssh-approve-text">
        <h2 id="ssh-approve-title">{t("developer.approveTitle")}</h2>
        <p id="ssh-approve-text">
          {request.program
            ? t("developer.approveTextProgram", { title: request.title, program: request.program })
            : t("developer.approveText", { title: request.title })}
        </p>
        <p>
          <code className="muted">{request.fingerprint}</code>
        </p>
        <div className="editor-actions">
          <button type="button" className="ghost" onClick={() => answer(false)}>
            {t("developer.deny")}
          </button>
          <button type="button" className="primary inline" autoFocus onClick={() => answer(true)}>
            {t("developer.allow")}
          </button>
        </div>
      </div>
    </div>
  );
}
