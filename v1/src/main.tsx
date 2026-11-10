import React, { lazy, Suspense } from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Splash from "./components/Splash";
import "./index.css";

// App 会引入所有业务页面，开发模式下首次编译很慢；懒加载让 splash 只加载自身。
const App = lazy(() => import("./App"));

// 应用内还没有主题切换功能，默认一律使用亮色主题，
// 不跟随操作系统的 prefers-color-scheme（否则暗色系统下部分用 CSS
// 变量驱动的组件会意外变暗）。等有了应用内主题设置后，再从那里控制
// 是否给 <html> 加 .dark。
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {getCurrentWindow().label === "splash" ? (
      <Splash />
    ) : (
      <Suspense fallback={null}>
        <App />
      </Suspense>
    )}
  </React.StrictMode>,
);
