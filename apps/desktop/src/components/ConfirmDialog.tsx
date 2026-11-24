import { useEffect, useRef } from "react";

/**
 * A confirmation popup over the whole window. The focus starts on Cancel (an Enter key meant for something else must
 * never confirm a deletion), Escape cancels, and a click outside cancels.
 */
export function ConfirmDialog({
  title,
  text,
  confirmLabel,
  cancelLabel,
  onConfirm,
  onCancel,
}: {
  title: string;
  text: string;
  confirmLabel: string;
  cancelLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const cancel = useRef<HTMLButtonElement>(null);
  useEffect(() => cancel.current?.focus(), []);
  return (
    <div className="modal-backdrop" onClick={onCancel}>
      <div
        className="modal"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        aria-describedby="confirm-text"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.key === "Escape" && onCancel()}
      >
        <h2 id="confirm-title">{title}</h2>
        <p id="confirm-text">{text}</p>
        <div className="editor-actions">
          <button type="button" className="ghost" ref={cancel} onClick={onCancel}>
            {cancelLabel}
          </button>
          <button type="button" className="danger" onClick={onConfirm}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
