// 拖拽排序：pointer 事件 + window 级监听。
// 不能把 move/up 挂在被拖元素上：换位会让 React 移动 DOM 节点，节点上的指针捕获随之丢失，
// 松开鼠标收不到 pointerup，表现为"松手后仍在拖"。不用 HTML5 draggable 的原因：
// tauri 开启文件拖放后 WebView 不再触发网页内的 drag/drop。

import { useEffect, useRef, useState } from "react";

export function usePointerReorder(attr: string, onMove: (from: string, to: string) => void) {
  const [drag, setDrag] = useState<{ id: string; x: number; y: number } | null>(null);
  const onMoveRef = useRef(onMove);
  onMoveRef.current = onMove;
  const stop = useRef<(() => void) | null>(null);
  const lastEnd = useRef(0);

  // 组件卸载时收尾，避免残留监听
  useEffect(() => () => stop.current?.(), []);

  const start = (id: string) => (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    stop.current?.();
    setDrag({ id, x: e.clientX, y: e.clientY });
    document.body.style.userSelect = "none";
    document.body.style.cursor = "grabbing";

    const move = (ev: PointerEvent) => {
      setDrag({ id, x: ev.clientX, y: ev.clientY });
      const over = document
        .elementFromPoint(ev.clientX, ev.clientY)
        ?.closest<HTMLElement>(`[${attr}]`)
        ?.getAttribute(attr);
      if (over && over !== id) onMoveRef.current(id, over);
    };
    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      window.removeEventListener("blur", end);
      document.body.style.userSelect = "";
      document.body.style.cursor = "";
      stop.current = null;
      lastEnd.current = Date.now();
      setDrag(null);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
    window.addEventListener("blur", end);
    stop.current = end;
  };

  // 松手后浏览器会补发 click（换位后落在被拖行上），调用方用它忽略这次 click
  const justDragged = () => Date.now() - lastEnd.current < 200;

  return { drag, start, justDragged };
}
