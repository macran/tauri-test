// 全量发票数据导入 —— 数据导入 / 税务数据
import { Receipt, CheckCircle, XCircle } from '@phosphor-icons/react'
import { commands, type ImportSummary, type SheetImportResult } from '@/bindings'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { cn } from '@/lib/utils'
import { ImportStatCard, StatusPill } from '@/components/tax-import/ImportFeedback'
import { ImportShell } from '@/components/tax-import/ImportShell'
import { errorText, useImportQueue } from '@/components/tax-import/useImportQueue'

export default function InvoiceImportPage() {
  const queue = useImportQueue<ImportSummary>(async ({ filePath }) => {
    try {
      const result = await commands.importInvoiceExcel({ filePath })
      return result.status === 'ok'
        ? { ok: true, summary: result.data }
        : { ok: false, message: result.error || '导入失败' }
    } catch (err) {
      return { ok: false, message: errorText(err) }
    }
  })

  return (
    <ImportShell<ImportSummary>
      title="全量发票导入"
      description="可一次选多个全量发票文件，每个文件独立成事务，失败只回滚该文件"
      icon={<Receipt className="mt-0.5 h-6 w-6 shrink-0 text-primary" weight="duotone" />}
      entries={queue.entries}
      onFiles={(files) => queue.enqueue(files, undefined)}
      onClear={queue.clearFinished}
      onRetry={(id) => queue.retry(id)}
      statusOf={(s) =>
        !s.committed
          ? { tone: 'error', text: '已回滚' }
          : s.sheets.some((sh) => sh.errors.length > 0)
            ? { tone: 'warning', text: '有错误' }
            : { tone: 'success', text: '导入成功' }
      }
      summaryLine={(s) =>
        s.committed
          ? `发票头 ${s.headerInserted.toLocaleString()} · 明细 ${s.detailInserted.toLocaleString()} · 供应商 ${s.sellerCount.toLocaleString()}`
          : (s.rollbackReason ?? '已回滚')
      }
      renderResult={(s) => <InvoiceImportResult summary={s} />}
    />
  )
}

function InvoiceImportResult({ summary }: { summary: ImportSummary }) {
  const hasErrors = summary.sheets.some((s) => s.errors.length > 0)

  return (
    <div className="flex flex-col gap-5">
      <div>
        <div className="grid grid-cols-3 gap-4">
          <ImportStatCard label="发票头" value={summary.headerInserted} />
          <ImportStatCard label="明细记录" value={summary.detailInserted} />
          <ImportStatCard label="供应商" value={summary.sellerCount} />
        </div>

        {summary.rollbackReason && (
          <Alert variant="destructive" className="mt-4">
            <XCircle weight="fill" />
            <AlertTitle>回滚原因</AlertTitle>
            <AlertDescription>{summary.rollbackReason}</AlertDescription>
          </Alert>
        )}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>各 Sheet 导入详情</CardTitle>
        </CardHeader>
        <CardContent className="px-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Sheet 名称</TableHead>
                <TableHead className="text-center">识别</TableHead>
                <TableHead className="text-center">导入记录</TableHead>
                <TableHead className="text-center">错误数</TableHead>
                <TableHead>状态</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {summary.sheets.map((sheet) => (
                <SheetRow key={sheet.sheetName} sheet={sheet} />
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      {hasErrors && (
        <Card>
          <CardHeader>
            <CardTitle className="text-[var(--accent-amber)]">错误单元格详情</CardTitle>
          </CardHeader>
          <CardContent className="px-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Sheet</TableHead>
                  <TableHead className="text-center">行</TableHead>
                  <TableHead className="text-center">列</TableHead>
                  <TableHead>错误信息</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {summary.sheets.flatMap((sheet) =>
                  sheet.errors.map((err, i) => (
                    <TableRow key={`${sheet.sheetName}-${i}`}>
                      <TableCell className="font-medium">{sheet.sheetName}</TableCell>
                      <TableCell className="text-center text-muted-foreground">{err.row}</TableCell>
                      <TableCell className="text-center font-mono text-muted-foreground">{err.col}</TableCell>
                      <TableCell className="text-[var(--accent-red)]">{err.message}</TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  )
}

function SheetRow({ sheet }: { sheet: SheetImportResult }) {
  const notRecognized = !sheet.recognized

  return (
    <TableRow>
      <TableCell className="font-medium">{sheet.sheetName}</TableCell>
      <TableCell className="text-center">
        {sheet.recognized ? (
          <CheckCircle className="mx-auto h-4.5 w-4.5 text-[var(--accent-green)]" weight="fill" />
        ) : (
          <XCircle className="mx-auto h-4.5 w-4.5 text-muted-foreground" />
        )}
      </TableCell>
      <TableCell className={cn('text-center tabular-nums', notRecognized && 'text-muted-foreground')}>
        {notRecognized ? '—' : sheet.inserted.toLocaleString()}
      </TableCell>
      <TableCell className="text-center">
        {sheet.errorCount > 0 ? (
          <span className="inline-flex items-center justify-center rounded-full bg-[var(--accent-red-bg)] px-2 py-0.5 text-xs font-semibold text-[var(--accent-red)]">
            {sheet.errorCount}
          </span>
        ) : (
          <span className="text-xs text-muted-foreground">—</span>
        )}
      </TableCell>
      <TableCell>
        {notRecognized ? (
          <StatusPill tone="info">未识别</StatusPill>
        ) : sheet.errorCount > 0 ? (
          <StatusPill tone="error">有错误</StatusPill>
        ) : sheet.inserted === 0 ? (
          <StatusPill tone="info">空 Sheet</StatusPill>
        ) : (
          <StatusPill tone="success">成功</StatusPill>
        )}
      </TableCell>
    </TableRow>
  )
}
