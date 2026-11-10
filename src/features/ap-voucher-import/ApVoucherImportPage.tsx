// src/features/ap-voucher-import/ApVoucherImportPage.tsx
// 应付凭证导入页。页面结构见 ImportShell：顶部操作栏（含接收日期 / 制表人）始终可用，结果累积成记录列表。

import { FileText } from "@phosphor-icons/react";
import { commands, type ApVoucherImportSummary } from "@/bindings";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ImportStatCard } from "@/components/tax-import/ImportFeedback";
import { ImportShell } from "@/components/tax-import/ImportShell";
import { errorText, useImportQueue } from "@/components/tax-import/useImportQueue";
import {
  VOUCHER_FORM_HINT,
  VoucherForm,
  useVoucherForm,
  type VoucherFormValue,
} from "@/features/voucher-upload/VoucherForm";

export default function ApVoucherImportPage() {
  const form = useVoucherForm();

  // 接收日期 / 制表人来自入队时固定的 extra，不读 form
  const queue = useImportQueue<ApVoucherImportSummary, VoucherFormValue>(
    async ({ filePath }, { receivedDate, preparer }) => {
      try {
        const result = await commands.importApVoucherExcel({ filePath, receivedDate, preparer });
        return result.status === "ok"
          ? { ok: true, summary: result.data }
          : { ok: false, message: result.error || "导入失败" };
      } catch (err) {
        return { ok: false, message: errorText(err) };
      }
    },
  );

  return (
    <ImportShell<ApVoucherImportSummary>
      title="应付凭证导入"
      description="导入应付凭证 Excel 文件，先填写接收日期和制表人"
      icon={<FileText className="mt-0.5 h-6 w-6 shrink-0 text-primary" weight="duotone" />}
      form={<VoucherForm form={form} />}
      disabled={!form.complete}
      disabledReason={VOUCHER_FORM_HINT}
      entries={queue.entries}
      onFiles={(files) => queue.enqueue(files, form.value)}
      onClear={queue.clearFinished}
      onRetry={(id) => queue.retry(id)}
      statusOf={(s) =>
        s.errorCount > 0
          ? { tone: "warning", text: "部分成功" }
          : { tone: "success", text: "导入完成" }
      }
      summaryLine={(s) =>
        `新插入 ${s.inserted} · 更新 ${s.updated} · 重复 ${s.skipped} · 错误 ${s.errorCount}`
      }
      renderResult={(s) => <ApVoucherImportResult summary={s} />}
    />
  );
}

function ApVoucherImportResult({ summary }: { summary: ApVoucherImportSummary }) {
  return (
    <div className="flex flex-col gap-5">
      <div className="grid grid-cols-2 gap-4 md:grid-cols-3">
        <ImportStatCard label="新插入（条）" value={summary.inserted} />
        <ImportStatCard label="凭证号已更新（红冲重做）（条）" value={summary.updated} />
        <ImportStatCard label="完全重复已忽略（条）" value={summary.skipped} />
        <ImportStatCard
          label="错误条数（条）"
          value={summary.errorCount}
          tone={summary.errorCount > 0 ? "error" : "default"}
        />
        <ImportStatCard label="新增供应商（家）" value={summary.vendorNewCount} />
        <ImportStatCard label="已有供应商（家）" value={summary.vendorExistingCount} />
      </div>

      {summary.errors.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-[var(--accent-amber)]">
              错误详情（共 {summary.errorCount} 条）
            </CardTitle>
          </CardHeader>
          <CardContent className="px-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="text-center">行号</TableHead>
                  <TableHead>Barcode</TableHead>
                  <TableHead>发票号码</TableHead>
                  <TableHead>错误类型</TableHead>
                  <TableHead>错误详情</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {summary.errors.map((err, i) => (
                  <TableRow key={i}>
                    <TableCell className="text-center text-muted-foreground">{err.row}</TableCell>
                    <TableCell className="font-mono text-muted-foreground">{err.barcode ?? "—"}</TableCell>
                    <TableCell className="font-mono text-muted-foreground">{err.invNum ?? "—"}</TableCell>
                    <TableCell>
                      <ErrorKindBadge kind={err.errorKind} />
                    </TableCell>
                    <TableCell className="text-[var(--accent-red)]">{err.message}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

// 文字色 + 同色半透明底，使用项目 accent 变量（亮 / 暗主题自动切换）
const RED = "bg-[var(--accent-red-bg)] text-[var(--accent-red)]";
const AMBER = "bg-[var(--accent-amber-bg)] text-[var(--accent-amber)]";
const VIOLET = "bg-[var(--accent-violet-bg)] text-[var(--accent-violet)]";
const TEAL = "bg-[var(--accent-teal-bg)] text-[var(--accent-teal)]";
const MUTED = "bg-muted text-muted-foreground";

const ERROR_KIND_COLORS: Record<string, string> = {
  必填字段为空: MUTED,
  金额解析失败: AMBER,
  发票类型非法: VIOLET,
  税务认证非法: TEAL,
  发票匹配失败: RED,
  金额核对失败: RED,
  发票类型核对失败: AMBER,
};

function ErrorKindBadge({ kind }: { kind: string }) {
  return (
    <span
      className={`inline-flex rounded-full px-2 py-0.5 text-xs font-medium ${ERROR_KIND_COLORS[kind] ?? MUTED}`}
    >
      {kind}
    </span>
  );
}
