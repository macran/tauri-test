// src/components/query/useQueryPage.ts
//
// 封装查询页面共有的状态机：加载态、错误态、结果集。不做分页——
// 后端一次性返回全部匹配行（有 MAX_ROWS 兜底上限），前端用虚拟列表渲染。

import { useCallback, useRef, useState } from "react";

type CommandResult<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: string };

export interface PageResultLike<R> {
  rows: R[];
  total: number;
}

export function useQueryPage<F extends Record<string, unknown>, R>(options: {
  queryFn: (filter: F) => Promise<CommandResult<PageResultLike<R>>>;
}) {
  const [rows, setRows] = useState<R[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hasSearched, setHasSearched] = useState(false);
  const lastFilter = useRef<F | null>(null);

  const search = useCallback(
    async (filter: F) => {
      lastFilter.current = filter;
      setLoading(true);
      setError(null);
      try {
        const result = await options.queryFn(filter);

        if (result.status === "ok") {
          setRows(result.data.rows);
          setTotal(result.data.total);
          setHasSearched(true);
        } else {
          setError(result.error || "查询失败");
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : "查询失败");
      } finally {
        setLoading(false);
      }
    },
    [options],
  );

  // 编辑/删除后用上次的条件静默重查：不进入 loading，表格不闪、滚动位置保留
  const refresh = useCallback(async () => {
    if (!lastFilter.current) return;
    const result = await options.queryFn(lastFilter.current);
    if (result.status === "ok") {
      setRows(result.data.rows);
      setTotal(result.data.total);
    }
  }, [options]);

  const truncated = rows.length < total;

  return {
    rows,
    total,
    truncated,
    loading,
    error,
    hasSearched,
    search,
    refresh,
  };
}
