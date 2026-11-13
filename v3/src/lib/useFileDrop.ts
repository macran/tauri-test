// 文件拖入窗口的统一入口。
//
// tauri.conf.json 里 dragDropEnabled 必须为 true：关闭时 WebView 给出的 File 没有磁盘路径，
// 后端读不到文件。开启后系统文件拖放由 Tauri 接管，网页内的 HTML5 drag/drop 不再触发，
// 因此这里用 onDragDropEvent 拿真实路径，并按落点坐标判断是否落在目标元素内。

import { useEffect, useRef, useState, type RefObject } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

const EXT_OF = (path: string) => path.split(".").pop()?.toLowerCase() ?? "";

export function useFileDrop(
  targetRef: RefObject<HTMLElement | null>,
  onFile: (filePath: string, fileName: string) => void,
  {
    disabled = false,
    extensions = ["xlsx", "xls"],
    onFiles,
  }: {
    disabled?: boolean;
    extensions?: string[];
    /** 一次拖入多个文件时拿到全部符合扩展名的路径；传了它就不再调用 onFile（onFile 只取第一个） */
    onFiles?: (filePaths: string[]) => void;
  } = {},
) {
  const [isDragging, setIsDragging] = useState(false);
  // 回调与开关用 ref 持有，避免每次渲染都重新订阅 Tauri 事件
  const onFileRef = useRef(onFile);
  const onFilesRef = useRef(onFiles);
  const disabledRef = useRef(disabled);
  const extRef = useRef(extensions);
  onFilesRef.current = onFiles;
  onFileRef.current = onFile;
  disabledRef.current = disabled;
  extRef.current = extensions;

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    const inside = (pos: { x: number; y: number }) => {
      const el = targetRef.current;
      if (!el) return false;
      const rect = el.getBoundingClientRect();
      const x = pos.x / window.devicePixelRatio;
      const y = pos.y / window.devicePixelRatio;
      return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
    };

    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (disabledRef.current) {
          setIsDragging(false);
          return;
        }
        if (p.type === "enter" || p.type === "over") {
          setIsDragging(inside(p.position));
        } else if (p.type === "drop") {
          setIsDragging(false);
          if (!inside(p.position)) return;
          const matched = p.paths.filter((f) => extRef.current.includes(EXT_OF(f)));
          if (matched.length === 0) return;
          if (onFilesRef.current) onFilesRef.current(matched);
          else onFileRef.current(matched[0], matched[0].split(/[\\/]/).pop() ?? matched[0]);
        } else {
          setIsDragging(false);
        }
      })
      .then((fn) => {
        // 组件在订阅完成前就卸载了：立刻取消，避免泄漏
        if (cancelled) fn();
        else unlisten = fn;
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [targetRef]);

  return isDragging;
}
