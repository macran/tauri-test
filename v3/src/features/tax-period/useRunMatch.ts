// 进项转出 / 印花税两个匹配页共用的状态：纳税所属期 + 来源期间表单、已入库结果的载入、匹配。
//
// 流程：
//   1. 选纳税所属期（单期或最多 3 个月）。切换时自动载入该所属期已入库的结果和来源期间。
//   2. 选来源期间（进项转出=认证期间，印花税=凭证期间），点「匹配」。
//      第一次匹配创建该所属期的批次；之后重复匹配仍是同一批次，不产生新批次。
//   3. 批次 1 待确认时改来源期间会清空本所属期已有的结果和人工修改：后端返回 RESET_REQUIRED 错误，
//      页面弹确认，用户同意后以 reset = true 重试。

import { useCallback, useEffect, useRef, useState } from "react";

import { type FieldErrors, matchFormSchema, validate } from "./schema";

export type Range = { from: string; to: string };

/** 后端要求二次确认时的错误：RESET_REQUIRED:{条数}|{说明} */
const RESET_PREFIX = "RESET_REQUIRED:";

/** 是「更改来源期间会清空已有数据」的错误则返回 { count, message }，否则 null */
export function parseResetRequired(error: unknown): { count: number; message: string } | null {
  const text = typeof error === "string" ? error : error instanceof Error ? error.message : "";
  if (!text.startsWith(RESET_PREFIX)) return null;
  const [count, ...rest] = text.slice(RESET_PREFIX.length).split("|");
  return { count: Number(count) || 0, message: rest.join("|") };
}

/** 页面注入的、与模块相关的部分 */
export interface MatchApi<R extends { exists: boolean; srcFrom: string | null; srcTo: string | null }, M> {
  /** 载入某纳税所属期已入库的结果（不匹配） */
  load: (filing: Range) => Promise<{ status: "ok"; data: R } | { status: "error"; error: unknown }>;
  /** 执行匹配 */
  match: (
    req: { filingFrom: string; filingTo: string; srcFrom: string; srcTo: string; reset: boolean },
  ) => Promise<{ status: "ok"; data: M } | { status: "error"; error: unknown }>;
  /** 从匹配结果里取出核对汇总 */
  reviewOf: (m: M) => R;
}

export function currentPeriod(): string {
  const now = new Date();
  return `${now.getFullYear()}${String(now.getMonth() + 1).padStart(2, "0")}`;
}

export function useRunMatch<R extends { exists: boolean; srcFrom: string | null; srcTo: string | null }, M>(
  api: MatchApi<R, M>,
) {
  const initial = currentPeriod();
  const [filing, setFiling] = useState<Range>({ from: initial, to: initial });
  const [src, setSrc] = useState<Range>({ from: initial, to: initial });
  const [errors, setErrors] = useState<FieldErrors>({});

  const [review, setReview] = useState<R | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  /** 每点一次「匹配」加一：表格据此清空搜索、列过滤和勾选 */
  const [runCount, setRunCount] = useState(0);
  /** 只认最后一次载入：连续切换时，晚到的旧响应不能盖掉新结果 */
  const loadSeq = useRef(0);

  const apiRef = useRef(api);
  apiRef.current = api;

  /** 载入该纳税所属期已入库的结果；已有批次时把来源期间回填到表单 */
  const loadStored = useCallback(async (f: Range) => {
    const seq = ++loadSeq.current;
    const result = await apiRef.current.load(f);
    if (seq !== loadSeq.current) return;
    if (result.status !== "ok" || !result.data.exists) {
      setReview(null);
      return;
    }
    setReview(result.data);
    const { srcFrom, srcTo } = result.data;
    if (srcFrom && srcTo) {
      setSrc({ from: srcFrom, to: srcTo });
    }
  }, []);

  useEffect(() => {
    void loadStored({ from: initial, to: initial });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function changeFiling(next: Range) {
    setFiling(next);
    setErrors({});
    setReview(null);
    // 来源期间默认跟随纳税所属期；已有批次时 loadStored 会用批次里保存的来源期间覆盖
    setSrc(next);
    void loadStored(next);
  }

  function changeSrc(next: Range) {
    setSrc(next);
    setErrors({});
  }

  function reset() {
    changeFiling({ from: initial, to: initial });
  }

  /** reset = true：用户已确认「更改来源期间会清空已有结果和人工修改」 */
  async function run(reset = false) {
    const checked = validate(matchFormSchema, {
      filingFrom: filing.from,
      filingTo: filing.to,
      srcFrom: src.from,
      srcTo: src.to,
    });
    if (!checked.ok) {
      setErrors(checked.errors);
      return null;
    }
    setErrors({});
    setIsRunning(true);
    loadSeq.current++;
    try {
      const result = await apiRef.current.match({ ...checked.data, reset });
      if (result.status === "ok") {
        setReview(apiRef.current.reviewOf(result.data));
        setRunCount((n) => n + 1);
      }
      return result;
    } finally {
      setIsRunning(false);
    }
  }

  return {
    filing,
    src,
    errors,
    review,
    setReview,
    isRunning,
    runCount,
    changeFiling,
    changeSrc,
    reset,
    run,
    /** 当前结果所属的纳税所属期；没有结果时跟随表单 */
    effectiveFiling: filing,
  };
}
