// src/components/query/QueryShell.tsx
//
// 查询页面统一外壳：标题 + 基础查询区（常驻）+ 高级查询区（默认收起，
// 点击展开）+ 查询/重置按钮 + 错误提示 + 结果表格区域。

import { MagnifyingGlass } from "@phosphor-icons/react";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

export interface QueryShellProps {
  title: string;
  description?: string;
  basicFields: React.ReactNode;
  advancedFields?: React.ReactNode;
  onSubmit: () => void;
  onReset: () => void;
  /** 表单底部左侧的布局下拉（LayoutSelect） */
  layout?: React.ReactNode;
  searching: boolean;
  error?: string | null;
  resultCount?: number;
  children: React.ReactNode;
}

export function QueryShell({
  title,
  description,
  basicFields,
  advancedFields,
  onSubmit,
  onReset,
  layout,
  searching,
  error,
  resultCount,
  children,
}: QueryShellProps) {
  return (
    <div className="mx-auto flex max-w-[1400px] flex-col gap-4 p-6">
      <Card>
        <CardHeader>
          <CardTitle>{title}</CardTitle>
          {description && <CardDescription>{description}</CardDescription>}
        </CardHeader>
        <CardContent>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              e.stopPropagation();
              onSubmit();
            }}
            className="flex flex-col gap-3"
          >
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
              {basicFields}
            </div>

            <Accordion>
              <AccordionItem value="advanced" className="border-b-0">
                {advancedFields && (
                  // keepMounted: 收起时高级字段仍挂载，保持输入值与表单状态
                  <AccordionContent keepMounted className="pb-3">
                    <div className="grid grid-cols-1 gap-3 border-t pt-3 sm:grid-cols-2 lg:grid-cols-4">
                      {advancedFields}
                    </div>
                  </AccordionContent>
                )}

                <div className="flex items-center justify-between pt-1">
                  {advancedFields ? (
                    <AccordionTrigger className="flex-none items-center gap-1 py-0 text-primary">
                      高级查询
                    </AccordionTrigger>
                  ) : (
                    <span />
                  )}

                  {layout && <div className="ml-4 mr-auto">{layout}</div>}

                  <div className="flex gap-2">
                    <Button type="button" variant="outline" onClick={onReset}>
                      重置
                    </Button>
                    <Button type="submit" disabled={searching}>
                      <MagnifyingGlass data-icon="inline-start" />
                      {searching ? "查询中…" : "查询"}
                    </Button>
                  </div>
                </div>
              </AccordionItem>
            </Accordion>

            {error && <p className="text-xs text-destructive">{error}</p>}
          </form>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center justify-between">
            <span>查询结果</span>
            {typeof resultCount === "number" && (
              <span className="text-xs font-normal text-muted-foreground">
                共 {resultCount} 条
              </span>
            )}
          </CardTitle>
        </CardHeader>
        <CardContent>{children}</CardContent>
      </Card>
    </div>
  );
}
