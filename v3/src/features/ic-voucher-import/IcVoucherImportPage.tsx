// src/features/ic-voucher-import/IcVoucherImportPage.tsx
// 内部关联方凭证导入页。页面结构见 ImportShell；接收日期 / 制表人在入队时固定（queue 的 extra）。

import { FileText, XCircle } from "@phosphor-icons/react";
import { commands, type IcVoucherImportSummary } from "@/bindings";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
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

type Result = IcVoucherImportSummary & VoucherFormValue;

// 各错误类型的语义色；未列出的用默认文字色
const KIND_COLOR: Record<string, string> = {
  发票未找到: "text-[var(--accent-red)]",
  税额不一致: "text-[var(--accent-amber)]",
  价税合计不一致: "text-[var(--accent-amber)]",
};

export default function IcVoucherImportPage() {
  const form = useVoucherForm();
  const queue = useImportQueue<Result, VoucherFormValue>(async ({ filePath }, extra) => {
    try {
      const result = await commands.importIcVoucherExcel({
        filePath,
        receivedDate: extra.receivedDate,
        preparer: extra.preparer,
      });
      return result.status === "ok"
        ? { ok: true, summary: { ...result.data, ...extra } }
        : { ok: false, message: result.error || "导入失败" };
    } catch (err) {
      return { ok: false, message: errorText(err) };
    }
  });

  return (
    <ImportShell<Result>
      title="内部关联方凭证导入"
      description="导入内部关联方凭证 Excel 文件，导入前请确保对应发票已入库"
      icon={<FileText className="mt-0.5 h-6 w-6 shrink-0 text-primary" weight="duotone" />}
      form={<VoucherForm form={form} />}
      disabled={!form.complete}
      disabledReason={VOUCHER_FORM_HINT}
      entries={queue.entries}
      onFiles={(files) => queue.enqueue(files, form.value)}
      onClear={queue.clearFinished}
      onRetry={(id) => queue.retry(id)}
      statusOf={(s) =>
        !s.committed
          ? { tone: "error", text: "已回滚" }
          : s.errorCount > 0
            ? { tone: "warning", text: "部分成功" }
            : { tone: "success", text: "导入成功" }
      }
      summaryLine={(s) =>
        `接收日期 ${s.receivedDate} · 制表人 ${s.preparer} · 写入 ${s.inserted} · 错误 ${s.errorCount}`
      }
      renderResult={(s) => <IcVoucherResult summary={s} />}
    />
  );
}

function IcVoucherResult({ summary }: { summary: Result }) {
  const kindCounts = Object.entries(
    summary.errors.reduce<Record<string, number>>((acc, e) => {
      acc[e.errorKind] = (acc[e.errorKind] ?? 0) + 1;
      return acc;
    }, {}),
  );

  return (
    <div className="flex flex-col gap-5">
      <div>
        <p className="mb-3 text-xs text-muted-foreground">
          接收日期：{summary.receivedDate}　制表人：{summary.preparer}
        </p>
        <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
          <ImportStatCard
            label={summary.committed ? "已写入数据库" : "写入记录（已回滚）"}
            value={summary.inserted}
            tone={summary.committed ? "success" : "default"}
          />
          <ImportStatCard
            label="错误条数"
            value={summary.errorCount}
            tone={summary.errorCount > 0 ? "error" : "default"}
          />
        </div>

        {!summary.committed && (
          <Alert variant="destructive" className="mt-4">
            <XCircle weight="fill" />
            <AlertTitle>已回滚</AlertTitle>
            <AlertDescription>
              导入过程中发现 {summary.errorCount} 条错误，所有数据已回滚，数据库未写入任何记录。请修正下方错误后重新导入。
              {summary.rollbackReason && <> 失败原因：{summary.rollbackReason}</>}
            </AlertDescription>
          </Alert>
        )}

        {kindCounts.length > 0 && (
          <div className="mt-4 flex flex-wrap gap-2">
            {kindCounts.map(([kind, count]) => (
              <span
                key={kind}
                className={`inline-flex items-center gap-1 rounded-full bg-muted px-2.5 py-0.5 text-xs font-medium ${KIND_COLOR[kind] ?? "text-muted-foreground"}`}
              >
                {kind}
                <span className="ml-0.5 font-bold">{count}</span>
              </span>
            ))}
          </div>
        )}
      </div>

      {summary.errors.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-[var(--accent-amber)]">错误详情（共 {summary.errorCount} 条）</CardTitle>
          </CardHeader>
          <CardContent className="px-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="text-center">行号</TableHead>
                  <TableHead>LE</TableHead>
                  <TableHead>发票号码</TableHead>
                  <TableHead>Document No.</TableHead>
                  <TableHead>错误类型</TableHead>
                  <TableHead>错误详情</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {summary.errors.map((err, i) => (
                  <TableRow key={i}>
                    <TableCell className="text-center text-muted-foreground">{err.row}</TableCell>
                    <TableCell className="text-muted-foreground">{err.legalEntity || "—"}</TableCell>
                    <TableCell className="font-mono text-muted-foreground">{err.invNum || "—"}</TableCell>
                    <TableCell className="font-mono text-muted-foreground">{err.documentNo || "—"}</TableCell>
                    <TableCell className={KIND_COLOR[err.errorKind] ?? "text-muted-foreground"}>{err.errorKind}</TableCell>
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
