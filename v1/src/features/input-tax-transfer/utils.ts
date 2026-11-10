/** 分（字符串）转元显示，带千分位。整数运算，不经过浮点。 */
export function fenToYuan(fen: string): string {
  const negative = fen.startsWith("-");
  const digits = (negative ? fen.slice(1) : fen).padStart(3, "0");
  const yuan = digits.slice(0, -2);
  const cents = digits.slice(-2);
  return `${negative ? "-" : ""}${Number(yuan).toLocaleString("zh-CN")}.${cents}`;
}

/** 分（字符串）转元，不带千分位，用于输入框回显。 */
export function fenToInput(fen: string): string {
  return fenToYuan(fen).replace(/,/g, "");
}

/** 用户输入的元转分，只接受非负、最多两位小数。非法输入返回 null。 */
export function yuanToFen(input: string): string | null {
  const trimmed = input.trim();
  if (!/^\d+(\.\d{0,2})?$/.test(trimmed)) return null;

  const [yuan, cents = ""] = trimmed.split(".");
  return String(BigInt(yuan) * 100n + BigInt(cents.padEnd(2, "0") || "0"));
}

export function getErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "未知错误";
}

export function currentPeriod(): string {
  const now = new Date();
  return `${now.getFullYear()}${String(now.getMonth() + 1).padStart(2, "0")}`;
}