// 凭证导入页共用的「接收日期 + 制表人」表单，放在 ImportShell 的顶部操作栏里。
// 两项任一为空时不能选文件（ImportShell 的 disabled）。
// 表单值在文件入队那一刻就固定下来（见 useImportQueue 的 extra）：用户在导入进行中改了表单，
// 不会影响已经排队的文件。

import { useState } from "react";

import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Win11DatePicker } from "@/components/ui/win11-date-picker";
import { parseLocalDate, toIsoDate } from "@/features/query/fields";

export type VoucherFormValue = { receivedDate: string; preparer: string };

export function useVoucherForm() {
  const [receivedDate, setReceivedDate] = useState("");
  const [preparer, setPreparer] = useState("");
  const value: VoucherFormValue = { receivedDate, preparer: preparer.trim() };
  return {
    value,
    complete: receivedDate !== "" && preparer.trim() !== "",
    receivedDate,
    preparer,
    setReceivedDate,
    setPreparer,
  };
}

export const VOUCHER_FORM_HINT = "请先填写接收日期和制表人";

export function VoucherForm({ form }: { form: ReturnType<typeof useVoucherForm> }) {
  return (
    <>
      <div className="flex flex-col gap-1">
        <Label className="text-xs text-muted-foreground">接收日期</Label>
        <Win11DatePicker
          value={parseLocalDate(form.receivedDate)}
          onChange={(d) => form.setReceivedDate(toIsoDate(d))}
          className="w-40"
        />
      </div>
      <div className="flex flex-col gap-1">
        <Label className="text-xs text-muted-foreground">制表人</Label>
        <Input
          value={form.preparer}
          onChange={(e) => form.setPreparer(e.target.value)}
          placeholder="请输入制表人"
          className="w-40"
        />
      </div>
    </>
  );
}
