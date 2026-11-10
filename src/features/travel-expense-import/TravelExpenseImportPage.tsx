// 差旅费用凭证文件导入页面
// 页面结构见 ImportShell：顶部操作栏始终可用，可一次选多个文件，结果累积成记录列表。
import { Airplane, XCircle } from '@phosphor-icons/react'
import { commands, type TravelExpenseImportSummary } from '@/bindings'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { ImportStatCard } from '@/components/tax-import/ImportFeedback'
import { ImportShell } from '@/components/tax-import/ImportShell'
import { errorText, useImportQueue } from '@/components/tax-import/useImportQueue'

export default function TravelExpenseImportPage() {
  const queue = useImportQueue<TravelExpenseImportSummary>(async ({ filePath }) => {
    try {
      const result = await commands.importTravelExpenseExcel({ filePath })
      return result.status === 'ok'
        ? { ok: true, summary: result.data }
        : { ok: false, message: result.error || '导入失败' }
    } catch (err) {
      return { ok: false, message: errorText(err) }
    }
  })

  return (
    <ImportShell<TravelExpenseImportSummary>
      title="差旅费用凭证导入"
      description="请先导入认证清单，再导入差旅费用凭证文件，用于更新凭证号码和入账时间"
      icon={<Airplane className="mt-0.5 h-6 w-6 shrink-0 text-primary" weight="duotone" />}
      entries={queue.entries}
      onFiles={(files) => queue.enqueue(files, undefined)}
      onClear={queue.clearFinished}
      onRetry={(id) => queue.retry(id)}
      statusOf={(s) =>
        !s.committed
          ? { tone: 'error', text: '已回滚' }
          : s.errorCount > 0
            ? { tone: 'warning', text: '部分成功' }
            : { tone: 'success', text: '导入成功' }
      }
      summaryLine={(s) => `成功更新 ${s.updated} 条 · 错误 ${s.errorCount} 条`}
      renderResult={(s) => <TravelExpenseResult summary={s} />}
    />
  )
}

function TravelExpenseResult({ summary }: { summary: TravelExpenseImportSummary }) {
  return (
    <div className="flex flex-col gap-5">
      <div>
        <div className="grid grid-cols-2 gap-4">
          <ImportStatCard label="成功更新（条）" value={summary.updated} />
          <ImportStatCard
            label="错误（条）"
            value={summary.errorCount}
            tone={summary.errorCount > 0 ? 'error' : 'default'}
          />
        </div>

        {summary.rollbackReason && (
          <Alert variant="destructive" className="mt-4">
            <XCircle weight="fill" />
            <AlertTitle>回滚原因</AlertTitle>
            <AlertDescription>{summary.rollbackReason}</AlertDescription>
          </Alert>
        )}
      </div>

      {summary.errorCount > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-[var(--accent-amber)]">错误明细（共 {summary.errorCount} 条）</CardTitle>
          </CardHeader>
          <CardContent className="px-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="text-center">Excel 行</TableHead>
                  <TableHead>inv_unq_key</TableHead>
                  <TableHead>凭证号码</TableHead>
                  <TableHead>错误信息</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {summary.errors.map((err, i) => (
                  <TableRow key={`${err.row}-${err.invUnqKey}-${i}`}>
                    <TableCell className="text-center text-muted-foreground">{err.row}</TableCell>
                    <TableCell className="font-mono text-muted-foreground">{err.invUnqKey || '—'}</TableCell>
                    <TableCell className="font-mono text-muted-foreground">{err.voucherNo || '—'}</TableCell>
                    <TableCell className="text-[var(--accent-red)]">{err.message}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
