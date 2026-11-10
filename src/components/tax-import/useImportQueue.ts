// 导入页共用的「导入队列」：一次可以选多个文件，按顺序逐个导入，结果累积成记录列表，不互相覆盖。
//
// 为什么必须串行：后端的数据库连接是单个，并且导入会开事务；两次导入同时进行既没有收益，也可能互相等锁。
// 所以任何时候最多一个文件在导入，其余在队列里等；导入进行中用户仍可继续选文件，新文件追加到队尾。

import { useCallback, useEffect, useRef, useState } from "react";

export type ImportEntry<S> = {
  id: number;
  fileName: string;
  filePath: string;
  /** 加入队列的时间，用来在列表里显示 */
  at: Date;
} & (
  | { status: "queued" }
  | { status: "loading" }
  | { status: "success"; summary: S }
  | { status: "error"; message: string }
);

export type ImportRunner<S, X> = (
  file: { filePath: string; fileName: string },
  extra: X,
) => Promise<{ ok: true; summary: S } | { ok: false; message: string }>;

const errorText = (err: unknown) =>
  typeof err === "string" ? err : ((err as Error)?.message ?? "未知错误");

/**
 * @param run 导入单个文件。返回 { ok:true, summary } 或 { ok:false, message }；抛出的异常也会被捕获成失败记录。
 *            `extra` 是入队时附带的额外参数（比如凭证页的接收日期和制表人）：入队时就固定下来，
 *            这样用户在导入进行中改了表单，也不会影响已经排队的文件。
 */
export function useImportQueue<S, X = void>(run: ImportRunner<S, X>) {
  const [entries, setEntries] = useState<ImportEntry<S>[]>([]);
  // 每条记录入队时的额外参数（如凭证页的接收日期 / 制表人），重试时原样复用，不读当前表单
  const extras = useRef(new Map<number, X>());
  const nextId = useRef(1);
  const queue = useRef<{ id: number; filePath: string; fileName: string; extra: X }[]>([]);
  const running = useRef(false);
  const runRef = useRef(run);
  runRef.current = run;
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const patch = useCallback((id: number, next: Partial<ImportEntry<S>> & { status: ImportEntry<S>["status"] }) => {
    if (!mounted.current) return;
    setEntries((prev) => prev.map((e) => (e.id === id ? ({ ...e, ...next } as ImportEntry<S>) : e)));
  }, []);

  const pump = useCallback(async () => {
    if (running.current) return;
    running.current = true;
    try {
      for (let job = queue.current.shift(); job; job = queue.current.shift()) {
        patch(job.id, { status: "loading" });
        try {
          const r = await runRef.current({ filePath: job.filePath, fileName: job.fileName }, job.extra);
          patch(job.id, r.ok ? { status: "success", summary: r.summary } : { status: "error", message: r.message });
        } catch (err) {
          patch(job.id, { status: "error", message: errorText(err) });
        }
      }
    } finally {
      running.current = false;
    }
  }, [patch]);

  /** 追加一批文件到队列；正在导入时也可以调用，新文件排在后面 */
  const enqueue = useCallback(
    (files: { filePath: string; fileName: string }[], extra: X) => {
      if (files.length === 0) return;
      const added = files.map((f) => ({ ...f, id: nextId.current++, extra }));
      for (const a of added) extras.current.set(a.id, extra);
      queue.current.push(...added);
      // 新记录放最前面：用户最关心最新的一条
      setEntries((prev) => [
        ...added.map((a) => ({ id: a.id, fileName: a.fileName, filePath: a.filePath, at: new Date(), status: "queued" as const })).reverse(),
        ...prev,
      ]);
      void pump();
    },
    [pump],
  );

  /** 重试一条失败的记录：作为新的一条重新入队，原来的失败记录保留（方便对照）。
   *  额外参数沿用这条记录入队时的值：接收日期、制表人这类会写进数据库的业务数据，
   *  不能因为用户重试前改了表单就悄悄变掉。 */
  const retry = useCallback(
    (id: number) => {
      const e = entries.find((x) => x.id === id);
      if (e && extras.current.has(id)) {
        enqueue([{ filePath: e.filePath, fileName: e.fileName }], extras.current.get(id) as X);
      }
    },
    [entries, enqueue],
  );

  const clearFinished = useCallback(() => {
    setEntries((prev) => {
      const keep = prev.filter((e) => e.status === "queued" || e.status === "loading");
      const ids = new Set(keep.map((e) => e.id));
      for (const id of [...extras.current.keys()]) if (!ids.has(id)) extras.current.delete(id);
      return keep;
    });
  }, []);

  const busy = entries.some((e) => e.status === "queued" || e.status === "loading");
  return { entries, enqueue, retry, clearFinished, busy };
}

export { errorText };
