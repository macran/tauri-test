// 系统管理：用户管理（仅 leader）/ 规则管理 / 备份与同步。
// 规则管理使用频率低、属于配置性质，所以放在这里；后端规则命令没有 leader 校验，因此标签对所有人开放。
// 用户管理的真正权限校验在后端每个 admin_* 命令里，这里只是不给非 leader 显示一个空白标签。

import { useEffect, useState } from "react";

import { commands } from "@/bindings";
import TabbedPage, { type TabEntry } from "@/components/TabbedPage";
import TaxRulesPage from "@/features/rules/TaxRulesPage";
import { useAppStore } from "@/store/useAppStore";

import BackupSync from "./BackupSync";
import UserManagement from "./UserManagement";

const COMMON: TabEntry[] = [
  { value: "rules", label: "规则管理", Component: TaxRulesPage },
  { value: "backup", label: "备份与同步", Component: BackupSync },
];

export default function SystemSettings() {
  const settingsTab = useAppStore((s) => s.settingsTab);
  const clearSettingsTab = useAppStore((s) => s.clearSettingsTab);
  // null = 还没读到身份。先不渲染，避免 leader 先看到没有“用户管理”标签、随后标签突然多出来
  const [isLeader, setIsLeader] = useState<boolean | null>(null);

  useEffect(() => {
    void commands.currentUserInfo().then((r) => setIsLeader(r.status === "ok" ? r.data.isLeader : false));
  }, []);

  if (isLeader === null) return null;

  const tabs: TabEntry[] = isLeader
    ? [{ value: "users", label: "用户管理", Component: UserManagement }, ...COMMON]
    : COMMON;

  return (
    <TabbedPage
      title="系统管理"
      tabs={tabs}
      requestedTab={settingsTab}
      onRequestedTabHandled={clearSettingsTab}
    />
  );
}
