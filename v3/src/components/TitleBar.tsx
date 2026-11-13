import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { cn } from "@/utils"; // 你自己的 cn
// import { LayoutGrid, Search, MinimizeIcon, MaximizeIcon, RestoreIcon, CloseIcon } from ...
import {
  CloseIcon,
  MaximizeIcon,
  MinimizeIcon,
  RestoreIcon,
} from "./icons/icons";

import {
  DiamondsFourIcon as LayoutGrid,
  MagnifyingGlassIcon as Search,
} from "@phosphor-icons/react"; // 需要安装 phosphor-react
export const TitleBar = () => {
  const [isMaximized, setIsMaximized] = useState(false);
  const [isFocused, setIsFocused] = useState(true);
  const appWindow = getCurrentWindow();

  useEffect(() => {
    appWindow.isMaximized().then(setIsMaximized);

    const unlistenResize = appWindow.onResized(async () => {
      setIsMaximized(await appWindow.isMaximized());
    });

    const unlistenFocus = appWindow.onFocusChanged(({ payload: focused }) => {
      setIsFocused(focused);
    });

    return () => {
      unlistenResize.then((f) => f());
      unlistenFocus.then((f) => f());
    };
  }, []);

  const hdlDoubleClic = async (e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest("button") || target.closest(".interactive-zone")) return;
    await appWindow.toggleMaximize();
  };

  // 统一标题栏前景色（文字/图标）
  const titlebarFgClass = cn(
    // active
    "text-[#171717] dark:text-[#f2f2f2]",

    // inactive
    !isFocused && "text-[#9b9b9b] dark:text-white/70",
  );

  // 统一按钮：继承前景色；hover 时“变重”
  const controlBtnClass = cn(
    "flex items-center justify-center w-11.5 h-full select-none",
    "transition-colors duration-150",
    "text-current",

    // hover（聚焦时：加重）
    isFocused &&
      "hover:text-[#171717] hover:bg-[#e9e9e9] dark:hover:text-white dark:hover:bg-[#2d2d2d]",

    // hover（未聚焦时：仍然比默认“重一些”，但保持灰阶）
    !isFocused &&
      "hover:text-[#171717] hover:bg-[#e9e9e9] dark:hover:text-white dark:hover:bg-[#2d2d2d]",

    // active（按下）
    "active:bg-black/10 dark:active:bg-white/15",
    "active:text-[#f2f2f2] dark:active:text-[#f2f2f2]",
  );

  const closeBtnClass = cn(
    "flex items-center justify-center w-11.5 h-full select-none",
    "transition-colors duration-150",
    "text-current",
    // 关闭按钮 hover：红底白字优先
    "hover:bg-[#e81123] hover:text-white",
    "active:bg-[#f1707a] dark:active:bg-[#8b0a15]",
  );

  return (
    <header
      data-tauri-drag-region
      onDoubleClick={hdlDoubleClic}
      className={cn(
        "flex items-center justify-between w-full",
        "z-50 sticky top-0",
        "select-none",
        "bg-transparent",
        isMaximized ? "h-8" : "h-9",

        titlebarFgClass,
      )}
    >
      <div className="flex items-center px-3 gap-3 pointer-events-none">
        <LayoutGrid
          size={16}
          className="text-blue-600 dark:text-blue-400"
          weight="fill"
        />
        <span className="text-[12px] font-medium text-current">My App</span>
      </div>

      <div className="flex-1 flex justify-center h-full items-center px-4 max-w-xl">
        <div
          className={cn(
            "interactive-zone flex items-center w-full max-w-sm h-6 px-2",
            "rounded-md border",
            "cursor-text transition-colors duration-150 pointer-events-auto",
            "bg-black/5 dark:bg-white/10 border-black/5 dark:border-white/5",
            "hover:bg-black/8 dark:hover:bg-white/15",
            // 搜索框内文字不跟随 titlebarFg（通常更浅一些更像占位符）
            "text-slate-500 dark:text-slate-400",
          )}
        >
          <Search size={12} className="mr-2 text-current" />
          <span className="text-[11px] text-current">Search</span>
        </div>
      </div>

      <div className="flex h-full items-stretch pointer-events-auto">
        <button
          className={controlBtnClass}
          onClick={() => appWindow.minimize()}
        >
          <MinimizeIcon className="text-current" />
        </button>

        <button
          className={controlBtnClass}
          onClick={() => appWindow.toggleMaximize()}
        >
          {isMaximized ? (
            <RestoreIcon className="text-current" />
          ) : (
            <MaximizeIcon className="text-current" />
          )}
        </button>

        <button className={closeBtnClass} onClick={() => appWindow.close()}>
          <CloseIcon className="text-current" />
        </button>
      </div>
    </header>
  );
};
