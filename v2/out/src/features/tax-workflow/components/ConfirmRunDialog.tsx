import { type FormEvent, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { fen, periodLabel, todayIso } from "../format";
import type { ConfirmInput, LineStats, TaxRun } from "../types";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  run: TaxRun | null;
  stats: LineStats | null;
  moduleLabel: string;
  taxLabel: string;
  onSubmit: (input: ConfirmInput) => Promise<void>;
}

/** 回填会计做账信息。纳税所属期已在创建匹配批次时确定，这里只确认并锁定。 */
export function ConfirmRunDialog({ open, onOpenChange, run, stats, moduleLabel, taxLabel, onSubmit }: Props) {
  const batch = run?.batches.find((b) => !b.confirmedAt);
  const needVoucher = (stats?.pendingCount ?? 0) > 0;
  const blocked = !run || !batch || !stats || !stats.exported || stats.exportStale || stats.needsReviewCount > 0 || stats.invalidCount > 0;
  const [voucherNo, setVoucherNo] = useState("");
  const [postedDate, setPostedDate] = useState("");
  const [preparer, setPreparer] = useState("");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (open) {
      setVoucherNo("");
      setPostedDate(needVoucher ? todayIso() : "");
      setPreparer("");
      setErrors({});
    }
  }, [open, run?.id, batch?.batchNo, needVoucher]);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!run) return;
    const next: Record<string, string> = {};
    if (needVoucher && !voucherNo.trim()) next.voucherNo = "请填写会计凭证号";
    if (needVoucher && !postedDate.trim()) next.postedDate = "请填写入账日期";
    if (needVoucher && !preparer.trim()) next.preparer = "请填写做账人";
    if (postedDate && !/^\d{4}-\d{2}-\d{2}$/.test(postedDate)) next.postedDate = "日期格式应为 YYYY-MM-DD";
    setErrors(next);
    if (Object.keys(next).length || blocked) return;
    setSubmitting(true);
    try {
      await onSubmit({
        runId: run.id,
        acctVoucherNo: voucherNo.trim() || null,
        acctPostedDate: postedDate.trim() || null,
        acctPreparer: preparer.trim() || null,
      });
    } finally {
      setSubmitting(false);
    }
  };

  const field = (key: "voucherNo" | "postedDate" | "preparer", label: string, value: string, set: (v: string) => void, type = "text") => (
    <div className="grid gap-1.5">
      <Label htmlFor={`confirm-${key}`}>{label}{needVoucher && <span className="ml-1 text-destructive">*</span>}</Label>
      <Input id={`confirm-${key}`} type={type} value={value} onChange={(e) => set(e.target.value)} aria-invalid={!!errors[key]} />
      {errors[key] && <p className="text-sm text-destructive" role="alert">{errors[key]}</p>}
    </div>
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        {run && stats && (
          <form onSubmit={(e) => void submit(e)} className="grid gap-4">
            <DialogHeader>
              <DialogTitle>确认并锁定{moduleLabel} · 批次 {batch?.batchNo ?? "-"}</DialogTitle>
              <DialogDescription>
                纳税所属期 {periodLabel(run.filingPeriod)}。本批 {stats.pendingCount} 行，{taxLabel} {fen(stats.pendingTax)} 元。确认后本所属期结果集锁定；如需更正，请由管理员解锁该所属期。
              </DialogDescription>
            </DialogHeader>
            <div className="rounded-md border bg-muted/30 p-3 text-sm">
              <div>本批导出状态：{stats.exported ? "已导出" : "尚未导出"}</div>
              {stats.exportStale && <div className="mt-1 text-destructive">导出后结果有变化，请重新导出交会计核对。</div>}
              {stats.needsReviewCount > 0 && <div className="mt-1 text-amber-700">还有 {stats.needsReviewCount} 行待复核。</div>}
              {stats.invalidCount > 0 && <div className="mt-1 text-destructive">还有 {stats.invalidCount} 行缺少有效印花税子目。</div>}
              {needVoucher && <div className="mt-1 text-muted-foreground">请填写会计团队返回的凭证信息；不需要在此选择纳税所属期。</div>}
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              {field("voucherNo", "会计凭证号", voucherNo, setVoucherNo)}
              {field("postedDate", "入账日期", postedDate, setPostedDate, "date")}
              <div className="sm:col-span-2">{field("preparer", "做账人", preparer, setPreparer)}</div>
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)} disabled={submitting}>取消</Button>
              <Button type="submit" disabled={blocked || submitting}>{submitting ? "确认中…" : "确认并锁定"}</Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}
