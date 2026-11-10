// 纳税所属期批次状态栏：待确认 / 已确认 / 已锁定，以及确认、取消确认、锁定（填凭证信息）、管理员解锁。
// 状态以后端为准：每次操作后用返回值更新，页面在匹配 / 保存之后调用 refresh() 重新读取。
//
//   待确认 ──确认──▶ 已确认 ──填凭证信息并锁定──▶ 已锁定
//          ◀─取消确认─┘                          └─管理员解锁─▶ 补充批次（待确认）

import { ArrowCounterClockwise, CheckCircle, LockKey, LockKeyOpen } from "@phosphor-icons/react";
import { type ReactNode, useCallback, useEffect, useId, useRef, useState } from "react";

import { type BatchState, commands, type PeriodStatus } from "@/bindings";
import { AppButton } from "@/components/app-button";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import { Win11DatePicker } from "@/components/ui/win11-date-picker";
import { cn } from "@/lib/utils";

import { type FieldErrors, lockFormSchema, validate } from "./schema";

export type Module = "input_tax_transfer" | "stamp_tax";

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "未知错误";
}

/** 读取并维护某模块某纳税所属期的批次状态。 */
export function usePeriodStatus(module: Module, filingFrom: string, filingTo: string) {
  const [status, setStatus] = useState<PeriodStatus | null>(null);
  // 只采用最后一次发起的读取：快速切换所属期时，慢的旧响应不能盖掉新所属期的状态
  const latest = useRef(0);

  const refresh = useCallback(async () => {
    const seq = ++latest.current;
    const result = await commands.getPeriodStatus({ module, filingFrom, filingTo });
    if (seq !== latest.current) return null;
    if (result.status !== "ok") {
      // 读取失败不能静默：空白状态栏看起来像「该所属期还没有批次」
      toast.add({ type: "error", title: "读取批次状态失败", description: errorMessage(result.error) });
      return null;
    }
    setStatus(result.data);
    return result.data;
  }, [module, filingFrom, filingTo]);

  // 切换纳税所属期时先清掉旧状态，避免短暂显示上一期间的结果
  useEffect(() => {
    setStatus(null);
    void refresh();
  }, [refresh]);

  return { status, setStatus, refresh };
}

const STATE_LABEL: Record<BatchState, string> = { open: "待确认", confirmed: "已确认", locked: "已锁定" };

const today = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};
const isoToDate = (iso: string) => {
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(y, m - 1, d);
};
const dateToIso = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;

type Dialogs = "confirm" | "lock" | "unlock" | null;

/** 确认前给用户核对的汇总：条数 + 金额（已格式化的元） */
export type ConfirmSummary = { rows: number; amount: string; amountLabel: string };

export function PeriodStatusBar({
  module,
  filingFrom,
  filingTo,
  status,
  onStatusChange,
  onConfirm,
  hasResult,
  blockedReason,
  extraActions,
  confirmSummary,
}: {
  module: Module;
  filingFrom: string;
  filingTo: string;
  status: PeriodStatus | null;
  /** 状态变化后（取消确认 / 锁定 / 解锁 / 确认）；页面据此重新读取核对表 */
  onStatusChange: (status: PeriodStatus) => void;
  /** 确认要写各模块自己的已确认快照，由页面传入对应命令 */
  onConfirm: () => Promise<{ status: "ok"; data: PeriodStatus } | { status: "error"; error: unknown }>;
  /** 本批次有可确认的内容（至少一条转出 / 计入，或补充批次有新增） */
  hasResult: boolean;
  /** 有必须处理的项（需人工核对、待选类别）时的原因，非空时不能确认；红色显示在按钮旁 */
  blockedReason?: string | null;
  /** 已确认 / 已锁定状态下的额外按钮（如印花税的「导出采集表」） */
  extraActions?: ReactNode;
  /** 传了就在确认前弹窗让用户核对条数和金额；不传则一点即确认 */
  confirmSummary?: ConfirmSummary;
}) {
  const [busy, setBusy] = useState(false);
  const [dialog, setDialog] = useState<Dialogs>(null);

  async function run(
    action: () => Promise<{ status: "ok"; data: PeriodStatus } | { status: "error"; error: unknown }>,
    doneTitle: string,
  ) {
    setBusy(true);
    try {
      const result = await action();
      if (result.status !== "ok") throw new Error(errorMessage(result.error));
      onStatusChange(result.data);
      toast.add({ type: "success", title: doneTitle });
      return true;
    } catch (error) {
      toast.add({ type: "error", title: "操作失败", description: errorMessage(error) });
      return false;
    } finally {
      setBusy(false);
    }
  }

  if (!status || !status.exists) return null;

  const key = { module, filingFrom, filingTo };
  const current = status.batches[status.batches.length - 1];
  const batchText = status.isSupplement ? `补充批次 ${status.batchNo}` : "批次 1";

  return (
    <div className="flex flex-wrap items-center gap-3 rounded-md border bg-muted/30 px-3 py-1.5 text-xs">
      <Badge
        // 三态各一种外观：待确认=空心，已确认=浅绿底，已锁定=实心主色；文字保持前景色以保证对比度
        variant={status.state === "locked" ? "default" : status.state === "open" ? "outline" : "secondary"}
        className={cn("gap-1 transition-colors duration-200 motion-reduce:transition-none", status.state === "confirmed" && "bg-[var(--accent-green-bg)] text-foreground")}
        aria-label={`${batchText} 状态：${STATE_LABEL[status.state]}`}
      >
        {status.state === "locked" ? (
          <LockKey key="locked" className="size-3 motion-safe:animate-in motion-safe:zoom-in-50 motion-safe:duration-200" />
        ) : status.state === "confirmed" ? (
          <CheckCircle className="size-3 text-[var(--accent-green)]" />
        ) : null}
        {batchText} · {STATE_LABEL[status.state]}
      </Badge>

      <span className="text-muted-foreground">
        {status.state === "open" &&
          (status.isSupplement
            ? `补充批次：${current?.openReason ?? ""}。已锁定的明细不会改动，确认后只含新增明细`
            : "确认后只读，可导出清单交会计；会计做完账后回填凭证信息并锁定")}
        {status.state === "confirmed" &&
          `${current?.confirmedBy} 于 ${current?.confirmedAt} 确认；导出清单交会计做账，做完后回填凭证信息并锁定`}
        {status.state === "locked" &&
          `${current?.voucherNo}（入账 ${current?.postedDate}，${current?.preparer}）· ${current?.lockedBy} 于 ${current?.lockedAt} 锁定`}
      </span>

      <div className="ml-auto flex items-center gap-2">
        {!!blockedReason && status.state === "open" && (
          <span role="status" className="font-medium text-destructive">{blockedReason}</span>
        )}
        {/* 禁用原因要看得见：title 在禁用按钮上键盘和触屏都看不到 */}
        {!blockedReason && !hasResult && status.state === "open" && (
          <span className="text-muted-foreground">
            {status.isSupplement ? "补充批次还没有新增明细" : "还没有可确认的明细，请先匹配"}
          </span>
        )}

        {status.state !== "open" && extraActions}

        {status.state === "open" && (
          <AppButton
            type="button"
            size="sm"
            variant="brand"
            disabled={busy || !hasResult || !!blockedReason}
            title={blockedReason ?? undefined}
            onClick={() => (confirmSummary ? setDialog("confirm") : void run(onConfirm, "已确认"))}
          >
            <CheckCircle data-icon="inline-start" />
            确认
          </AppButton>
        )}

        {status.state === "confirmed" && (
          <>
            <AppButton
              type="button"
              size="sm"
              variant="glass"
              disabled={busy}
              onClick={() => void run(() => commands.unconfirmTaxPeriod(key), "已取消确认，可以继续修改")}
            >
              <ArrowCounterClockwise data-icon="inline-start" />
              取消确认
            </AppButton>
            <AppButton type="button" size="sm" variant="danger" disabled={busy} onClick={() => setDialog("lock")}>
              <LockKey data-icon="inline-start" />
              填写凭证信息并锁定
            </AppButton>
          </>
        )}

        {status.state === "locked" && status.canUnlock && (
          <AppButton type="button" size="sm" variant="glass" disabled={busy} onClick={() => setDialog("unlock")}>
            <LockKeyOpen data-icon="inline-start" />
            解锁（补充）
          </AppButton>
        )}

        {/* 补充批次还没有新增时，管理员可以撤销这个空批次 */}
        {status.state === "open" && status.isSupplement && status.canUnlock && (
          <AppButton
            type="button"
            size="sm"
            variant="glass"
            disabled={busy}
            onClick={() => void run(() => commands.relockTaxPeriod(key), "已重新锁定")}
          >
            <LockKey data-icon="inline-start" />
            重新锁定
          </AppButton>
        )}
      </div>

      {dialog === "confirm" && confirmSummary && (
        <AlertDialog open onOpenChange={(open) => !open && !busy && setDialog(null)}>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>
                确认{filingFrom === filingTo ? filingFrom : `${filingFrom}-${filingTo}`} {batchText}
              </AlertDialogTitle>
              <AlertDialogDescription>
                将确认 {confirmSummary.rows} 条，{confirmSummary.amountLabel} {confirmSummary.amount} 元。
                确认后这批明细只读，可导出清单交会计；做账前可以随时取消确认继续修改。
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel disabled={busy}>取消</AlertDialogCancel>
              <AlertDialogAction
                disabled={busy}
                onClick={(e) => {
                  e.preventDefault();
                  void run(onConfirm, "已确认").then((ok) => ok && setDialog(null));
                }}
              >
                确认 {confirmSummary.rows} 条，合计 {confirmSummary.amount} 元
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      )}
      {dialog === "lock" && (
        <LockDialog
          label={module === "stamp_tax" ? "印花税" : "进项转出"}
          busy={busy}
          onClose={() => setDialog(null)}
          onSubmit={async (v) => {
            const ok = await run(() => commands.lockTaxPeriod({ ...key, ...v }), "已锁定");
            if (ok) setDialog(null);
          }}
        />
      )}
      {dialog === "unlock" && (
        <UnlockDialog
          busy={busy}
          onClose={() => setDialog(null)}
          onSubmit={async (reason) => {
            const ok = await run(() => commands.unlockTaxPeriod({ ...key, reason }), "已解锁，已开启补充批次");
            if (ok) setDialog(null);
          }}
        />
      )}
    </div>
  );
}

/** 锁定：填写本批进项转出 / 印花税自己的会计凭证信息（不是 AP/IC/银行/差旅的原记账凭证）。 */
function LockDialog({
  label,
  busy,
  onClose,
  onSubmit,
}: {
  label: string;
  busy: boolean;
  onClose: () => void;
  onSubmit: (v: { voucherNo: string; postedDate: string; preparer: string }) => Promise<void>;
}) {
  const [voucherNo, setVoucherNo] = useState("");
  const [postedDate, setPostedDate] = useState(today());
  const [preparer, setPreparer] = useState("");
  const [errors, setErrors] = useState<FieldErrors>({});
  const uid = useId();

  return (
    <Dialog open onOpenChange={(open) => !open && !busy && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>锁定并填写{label}凭证信息</DialogTitle>
        </DialogHeader>
        <form
          className="flex flex-col gap-3"
          onSubmit={(e) => {
            e.preventDefault();
            const checked = validate(lockFormSchema, { voucherNo, postedDate, preparer });
            if (!checked.ok) return setErrors(checked.errors);
            setErrors({});
            void onSubmit(checked.data);
          }}
        >
          <p className="text-xs text-muted-foreground">
            填写会计做账后的{label}会计凭证信息。锁定后这批明细永久不可修改，需要补充时由管理员解锁。
          </p>
          <div className="flex flex-col gap-1">
            <Label htmlFor={`${uid}-voucherNo`} className="text-muted-foreground">{label}会计凭证号</Label>
            <Input
              id={`${uid}-voucherNo`}
              value={voucherNo}
              aria-invalid={!!errors.voucherNo}
              aria-describedby={errors.voucherNo ? `${uid}-voucherNo-err` : undefined}
              onChange={(e) => setVoucherNo(e.target.value)}
            />
            {errors.voucherNo && (
              <p id={`${uid}-voucherNo-err`} role="alert" className="text-xs text-destructive">{errors.voucherNo}</p>
            )}
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor={`${uid}-postedDate`} className="text-muted-foreground">入账日期</Label>
            <Win11DatePicker
              id={`${uid}-postedDate`}
              aria-invalid={!!errors.postedDate}
              aria-describedby={errors.postedDate ? `${uid}-postedDate-err` : undefined}
              clearable={false}
              value={isoToDate(postedDate)}
              onChange={(d) => d && setPostedDate(dateToIso(d))}
            />
            {errors.postedDate && (
              <p id={`${uid}-postedDate-err`} role="alert" className="text-xs text-destructive">{errors.postedDate}</p>
            )}
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor={`${uid}-preparer`} className="text-muted-foreground">做账人</Label>
            <Input
              id={`${uid}-preparer`}
              value={preparer}
              aria-invalid={!!errors.preparer}
              aria-describedby={errors.preparer ? `${uid}-preparer-err` : undefined}
              onChange={(e) => setPreparer(e.target.value)}
            />
            {errors.preparer && (
              <p id={`${uid}-preparer-err`} role="alert" className="text-xs text-destructive">{errors.preparer}</p>
            )}
          </div>
          <DialogFooter>
            <AppButton type="button" variant="glass" disabled={busy} onClick={onClose}>
              取消
            </AppButton>
            <AppButton type="submit" variant="danger" disabled={busy}>
              确认锁定
            </AppButton>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function UnlockDialog({
  busy,
  onClose,
  onSubmit,
}: {
  busy: boolean;
  onClose: () => void;
  onSubmit: (reason: string) => Promise<void>;
}) {
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string | null>(null);
  const uid = useId();

  return (
    <Dialog open onOpenChange={(open) => !open && !busy && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>解锁并开启补充批次</DialogTitle>
        </DialogHeader>
        <form
          className="flex flex-col gap-3"
          onSubmit={(e) => {
            e.preventDefault();
            if (!reason.trim()) return setError("请填写解锁原因");
            setError(null);
            void onSubmit(reason.trim());
          }}
        >
          <p className="text-xs text-muted-foreground">
            已锁定的明细不会改动。补充的明细仍归本纳税所属期，单独作为补充批次，会计另行做账。
          </p>
          <div className="flex flex-col gap-1">
            <Label htmlFor={`${uid}-reason`} className="text-muted-foreground">解锁原因</Label>
            <Textarea
              id={`${uid}-reason`}
              aria-describedby={error ? `${uid}-reason-err` : undefined}
              value={reason}
              placeholder="例如：税务局核查补税"
              aria-invalid={!!error}
              onChange={(e) => setReason(e.target.value)}
            />
            {error && <p id={`${uid}-reason-err`} role="alert" className="text-xs text-destructive">{error}</p>}
          </div>
          <DialogFooter>
            <AppButton type="button" variant="glass" disabled={busy} onClick={onClose}>
              取消
            </AppButton>
            <AppButton type="submit" variant="brand" disabled={busy}>
              确认解锁
            </AppButton>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
