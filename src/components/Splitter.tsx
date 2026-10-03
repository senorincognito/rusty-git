import { useRef } from "react";
import { t } from "@/i18n";
import "./Splitter.scss";

/**
 * A horizontal bar between two stacked areas, dragged up and down. The owner reads the areas' sizes in `onStart`
 * and applies the movement (in px, relative to where the drag began) in `onMove`; ↑/↓ nudge by `onNudge`,
 * double-click calls `onReset`. `onEnd` fires once when a drag or nudge is finished (the place to save).
 */
export default function Splitter({
  onStart,
  onMove,
  onEnd,
  onNudge,
  onReset,
}: {
  onStart: () => void;
  onMove: (dy: number) => void;
  onEnd: () => void;
  onNudge: (dy: number) => void;
  onReset: () => void;
}) {
  const startY = useRef<number | null>(null);

  const end = () => {
    if (startY.current === null) return;
    startY.current = null;
    document.body.classList.remove("resizing-v");
    onEnd();
  };

  return (
    <div
      className="splitter"
      role="separator"
      aria-orientation="horizontal"
      aria-label={t.splitter.label}
      tabIndex={0}
      title={t.splitter.hint}
      onPointerDown={(e) => {
        e.preventDefault();
        e.currentTarget.setPointerCapture(e.pointerId);
        startY.current = e.clientY;
        document.body.classList.add("resizing-v");
        onStart();
      }}
      onPointerMove={(e) => startY.current !== null && onMove(e.clientY - startY.current)}
      onPointerUp={end}
      onPointerCancel={end}
      onDoubleClick={onReset}
      onKeyDown={(e) => {
        const step = e.shiftKey ? 48 : 16;
        if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
        e.preventDefault();
        onStart();
        onNudge(e.key === "ArrowDown" ? step : -step);
        onEnd();
      }}
    />
  );
}
