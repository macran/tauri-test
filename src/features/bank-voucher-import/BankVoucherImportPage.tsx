// src/features/bank-voucher-import/BankVoucherImportPage.tsx
//
// 银行费用凭证导入页。页面结构见 ImportShell：顶部操作栏始终可用，可一次选多个文件，结果累积成记录列表。

import { Receipt, XCircle } from "@phosphor-icons/react";
import {
  commands,
  type BankVoucherImportSummary,
  type BankVoucherImportError,
} from "@/bindings";
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

const hasAnyError = (s: BankVoucherImportSummary) =>
  s.notFoundCount > 0 || s.taxMismatchCount > 0 || s.ambiguousCount > 0 || s.parseErrorCount > 0;

export default function BankVoucherImportPage() {
  const form = useVoucherForm();

  // 表单值来自入队时固定的 extra，不在这里读 form
  const queue = useImportQueue<BankVoucherImportSummary, VoucherFormValue>(
    async ({ filePath }, extra) => {
      try {
        const result = await commands.importBankVoucherExcel({
          filePath,
          receivedDate: extra.receivedDate,
          preparedBy: extra.preparer,
        });
        return result.status === "ok"
          ? { ok: true, summary: result.data }
          : { ok: false, message: result.error || "银行费用凭证导入失败" };
      } catch (err) {
        return { ok: false, message: errorText(err) };
      }
    },
  );

  return (
    <ImportShell<BankVoucherImportSummary>
      title="银行费用凭证导入"
      description="导入银行费用凭证 Excel 文件，需先填写接收日期和制表人"
      icon={<Receipt className="mt-0.5 h-6 w-6 shrink-0 text-primary" weight="duotone" />}
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
          : hasAnyError(s)
            ? { tone: "warning", text: "部分成功" }
            : { tone: "success", text: "已提交" }
      }
      summaryLine={(s) =>
        `成功 ${s.inserted} · 未找到 ${s.notFoundCount} · 税额不一致 ${s.taxMismatchCount} · 重复匹配 ${s.ambiguousCount} · 解析错误 ${s.parseErrorCount}`
      }
      renderResult={(s) => <ImportResult summary={s} />}
    />
  );
}

function ImportResult({ summary }: { summary: BankVoucherImportSummary }) {
  return (
    <div className="flex flex-col gap-5">
      <div className="grid grid-cols-2 gap-4 md:grid-cols-5">
        <ImportStatCard label="成功写入" value={summary.inserted} />
        <ImportStatCard
          label="未找到"
          value={summary.notFoundCount}
          tone={summary.notFoundCount > 0 ? "warning" : "default"}
        />
        <ImportStatCard
          label="税额不一致"
          value={summary.taxMismatchCount}
          tone={summary.taxMismatchCount > 0 ? "warning" : "default"}
        />
        <ImportStatCard
          label="重复匹配"
          value={summary.ambiguousCount}
          tone={summary.ambiguousCount > 0 ? "warning" : "default"}
        />
        <ImportStatCard
          label="解析错误"
          value={summary.parseErrorCount}
          tone={summary.parseErrorCount > 0 ? "error" : "default"}
        />
      </div>

      {summary.rollbackReason && (
        <Alert variant="destructive">
          <XCircle weight="fill" />
          <AlertTitle>回滚原因</AlertTitle>
          <AlertDescription>{summary.rollbackReason}</AlertDescription>
        </Alert>
      )}

      {summary.errors.length > 0 && <ErrorTable errors={summary.errors} />}
    </div>
  );
}

function ErrorTable({ errors }: { errors: BankVoucherImportError[] }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-[var(--accent-amber)]">错误详情（共 {errors.length} 条）</CardTitle>
      </CardHeader>
      <CardContent className="px-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="text-center">行</TableHead>
              <TableHead>类型</TableHead>
              <TableHead>发票号码</TableHead>
              <TableHead>凭证号码</TableHead>
              <TableHead>错误信息</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {errors.map((err, index) => (
              <TableRow key={`${err.row}-${index}`}>
                <TableCell className="text-center text-muted-foreground">{err.row}</TableCell>
                <TableCell className="text-muted-foreground">{formatErrorKind(err.kind)}</TableCell>
                <TableCell className="font-mono">{err.invoiceNo || "—"}</TableCell>
                <TableCell className="font-mono">{err.voucherNo || "—"}</TableCell>
                <TableCell className="text-[var(--accent-red)]">{err.message}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

function formatErrorKind(kind: BankVoucherImportError["kind"]) {
  switch (kind) {
    case "parse":
      return "解析错误";
    case "notFound":
      return "未找到";
    case "taxMismatch":
      return "税额不一致";
    case "ambiguous":
      return "重复匹配";
    default:
      return kind;
  }
}
