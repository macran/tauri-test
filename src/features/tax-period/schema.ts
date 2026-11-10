// 进项税转出 / 印花税 两个匹配页共用的表单校验。
// 纳税所属期：单期或最多连续 3 个自然月（YYYYMM）。来源期间（进项转出=认证期间，印花税=凭证期间）：
// 起 ≤ 止，且止不晚于纳税所属期止。

import { z } from "zod";

const PERIOD = /^\d{4}(0[1-9]|1[0-2])$/;

export const periodSchema = z.string().regex(PERIOD, "期间格式应为 YYYYMM，例如 202610");

/** "202601" -> 2026*12+0；用来算相差几个月 */
const monthIndex = (p: string) => Number(p.slice(0, 4)) * 12 + Number(p.slice(4, 6)) - 1;

/** 匹配表单：纳税所属期 + 来源期间。进项转出和印花税结构相同，只是来源期间的叫法不同。 */
export const matchFormSchema = z
  .object({
    filingFrom: periodSchema,
    filingTo: periodSchema,
    srcFrom: periodSchema,
    srcTo: periodSchema,
  })
  .superRefine((v, ctx) => {
    if (v.filingFrom > v.filingTo) {
      ctx.addIssue({ code: "custom", path: ["filingTo"], message: "结束期间不能早于起始期间" });
    } else if (monthIndex(v.filingTo) - monthIndex(v.filingFrom) > 2) {
      ctx.addIssue({ code: "custom", path: ["filingTo"], message: "纳税所属期最多跨 3 个自然月" });
    }
    if (v.srcFrom > v.srcTo) {
      ctx.addIssue({ code: "custom", path: ["srcTo"], message: "结束期间不能早于起始期间" });
    } else if (v.srcTo > v.filingTo) {
      ctx.addIssue({ code: "custom", path: ["srcTo"], message: "不能晚于纳税所属期的结束期间" });
    }
  });

/** 查询：纳税所属期起 ≤ 止 */
export const historyRangeSchema = z
  .object({ periodFrom: periodSchema, periodTo: periodSchema })
  .superRefine((v, ctx) => {
    if (v.periodFrom > v.periodTo) {
      ctx.addIssue({ code: "custom", path: ["periodTo"], message: "结束期间不能早于起始期间" });
    }
  });

/** 锁定：本批进项转出 / 印花税的会计凭证信息，三项必填 */
export const lockFormSchema = z.object({
  voucherNo: z.string().trim().min(1, "请填写会计凭证号"),
  postedDate: z
    .string()
    .regex(/^\d{4}-\d{2}-\d{2}$/, "请选择入账日期")
    .refine((v) => {
      // 全程按 UTC 解析和回写：用本地时间解析再 toISOString 会在东八区差一天
      const d = new Date(`${v}T00:00:00Z`);
      return !Number.isNaN(d.getTime()) && d.toISOString().slice(0, 10) === v;
    }, "日期不存在"),
  preparer: z.string().trim().min(1, "请填写做账人"),
});

export type FieldErrors = Partial<Record<string, string>>;

/** 校验并整理成「字段名 → 第一条错误」，页面直接按字段显示。 */
export function validate<T extends z.ZodType>(
  schema: T,
  value: unknown,
): { ok: true; data: z.infer<T> } | { ok: false; errors: FieldErrors } {
  const result = schema.safeParse(value);
  if (result.success) return { ok: true, data: result.data };

  const errors: FieldErrors = {};
  for (const issue of result.error.issues) {
    const key = String(issue.path[0] ?? "form");
    errors[key] ??= issue.message;
  }
  return { ok: false, errors };
}
