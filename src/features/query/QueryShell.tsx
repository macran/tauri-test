// src/components/query/QueryShell.tsx
//
// 查询页面统一外壳：标题 + 基础查询区（常驻）+ 高级查询区（默认收起，
// 点击展开）+ 查询/重置按钮 + 错误提示 + 结果表格区域。

import { useState } from "react";
import { ArrowCounterClockwise, CaretDown, CaretUp, CircleNotch, MagnifyingGlass } from "@phosphor-icons/react";
import { AppButton } from "@/components/app-button";
import { Card, CardContent } from "@/components/ui/card";

export interface QueryShellProps {
  basicFields: React.ReactNode;
  advancedFields?: React.ReactNode;
  /** 高级查询里已生效的条件数（如批量查询的字段数）；折叠时显示在「高级查询」后，免得用户忘了还有条件在过滤 */
  advancedActiveCount?: number;
  onSubmit: () => void;
  onReset: () => void;
  /** 布局下拉（LayoutSelect）：和其他字段一样是表单字段，排在字段末尾 */
  layout?: React.ReactNode;
  /** 帮助构造查询条件的工具（如批量查询），和「高级查询」排在一起 */
  queryTools?: React.ReactNode;
  /** 其他附加按钮，排在重置前 */
  extraActions?: React.ReactNode;
  searching: boolean;
  error?: string | null;
  children: React.ReactNode;
}

export function QueryShell({
  basicFields,
  advancedFields,
  advancedActiveCount = 0,
  onSubmit,
  onReset,
  layout,
  queryTools,
  extraActions,
  searching,
  error,
  children,
}: QueryShellProps) {
  const [advancedOpen, setAdvancedOpen] = useState(false);

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col gap-3 p-3">
      <Card>
        <CardContent>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              e.stopPropagation();
              onSubmit();
            }}
            className="flex flex-col gap-2"
          >
            {/* 字段按固定宽度流式排列：窗口够宽时同一行放下，窄了自动换行而不是压缩输入框。
                布局是字段，跟在字段后面；命令区靠右分两组：[高级查询 + 批量查询] 是构造条件的，[重置 + 查询] 是执行的。 */}
            <div className="flex flex-wrap items-end gap-x-3 gap-y-2">
              {basicFields}
              {layout}

              <div className="ml-auto flex items-center gap-4">
                <div className="flex items-center gap-3">
                  {advancedFields && (
                    <button
                      type="button"
                      aria-expanded={advancedOpen}
                      onClick={() => setAdvancedOpen((v) => !v)}
                      className="inline-flex h-(--control-h) items-center gap-1 text-xs font-medium text-primary hover:underline"
                    >
                      高级查询
                      {!advancedOpen && advancedActiveCount > 0 && (
                        <span className="rounded-full bg-primary px-1.5 text-[10px] leading-4 text-primary-foreground">
                          {advancedActiveCount}
                        </span>
                      )}
                      {advancedOpen ? <CaretUp className="size-3" /> : <CaretDown className="size-3" />}
                    </button>
                  )}
                  {queryTools}
                </div>
                <div className="flex gap-2">
                  {extraActions}
                  <AppButton type="button" variant="glass" onClick={onReset}>
                    <ArrowCounterClockwise data-icon="inline-start" />
                    重置
                  </AppButton>
                  <AppButton type="submit" variant="brand" disabled={searching}>
                    {/* 文字不变、只换图标：按钮宽度恒定，左侧元素不会被挤动 */}
                    {searching ? (
                      <CircleNotch data-icon="inline-start" className="animate-spin" />
                    ) : (
                      <MagnifyingGlass data-icon="inline-start" />
                    )}
                    查询
                  </AppButton>
                </div>
              </div>
            </div>

            {/* hidden 而不是卸载：收起时高级字段仍挂载，保持输入值与表单状态 */}
            {advancedFields && (
              <div
                hidden={!advancedOpen}
                className="flex flex-wrap items-end gap-x-3 gap-y-2 border-t pt-2"
              >
                {advancedFields}
              </div>
            )}

            {error && <p className="text-xs text-destructive">{error}</p>}
          </form>
        </CardContent>
      </Card>

      <Card className="min-h-0 flex-1">
        <CardContent className="flex min-h-0 flex-1 flex-col gap-2">
          {children}
        </CardContent>
      </Card>
    </div>
  );
}
