// src/components/query/format.ts
//
// 查询结果展示用的格式化辅助函数。
// 金额字段后端以“分”为单位的字符串传输（避免超出前端 Number 精度），
// 这里统一转换为“元”展示，保留两位小数、千分位分隔。

export function formatMoney(cents: string | null | undefined): string {
  if (cents === null || cents === undefined || cents === "") return "—";
  const n = Number(cents);
  if (Number.isNaN(n)) return cents;
  return (n / 100).toLocaleString("zh-CN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

export function displayOrDash(value: string | null | undefined): string {
  if (value === null || value === undefined || value.trim() === "") return "—";
  return value;
}
