import { useEffect, type ReactNode } from "react";
import { createPortal } from "react-dom";

/** A centred dialog over a dimmed backdrop. Escape or clicking the backdrop calls `onClose`. */
export default function Modal({
  title,
  onClose,
  width = 560,
  children,
}: {
  title: string;
  onClose: () => void;
  width?: number;
  children: ReactNode;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  return createPortal(
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal" role="dialog" aria-modal="true" aria-label={title} style={{ width }}>
        <h2>{title}</h2>
        {children}
      </div>
    </div>,
    document.body,
  );
}
