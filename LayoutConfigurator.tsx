/* =============================================================================
 * LayoutConfigurator —— SAP 式表格布局配置器 (v3)
 * =============================================================================
 *
 * 数据模型要点:
 *   - LayoutConfig.columns 是「有序的可见列数组」,顺序本身就是唯一真相
 *   - 前 frozen 个元素即冻结列 —— “固定列在前面” 是结构上的不变量
 *   - 不存 isVisible / order / name / table:
 *       在 columns 里 = 可见,不在 = 备选
 *       字段的中文名与所属表来自代码里的 QUERY_REGISTRY,绝不写进 config_json
 *
 * v3 修复:
 *   - Row 提到模块顶层并 memo —— 之前定义在组件内部,dragover 期间每次
 *     setState 都会生成新组件类型导致子树卸载重建,拖拽被浏览器中止
 *   - onDragStart 补 dataTransfer.setData (WebView2 / WebKit 需要)
 *   - 补 onDrop + 容器 onDragOver preventDefault
 *   - 移除键盘 ↑↓ 排序(会和页面滚动抢按键),行不再抢 tab 焦点
 * ========================================================================== */

import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowRight,
  ArrowUUpLeft,
  CaretDown,
  CaretUp,
  CheckCircle,
  DotsSixVertical,
  FloppyDisk,
  Lock,
  MagnifyingGlass,
  Play,
  PushPin,
  Table as TableIcon,
  X,
} from "@phosphor-icons/react";

/* ===========================================================================
 * 1. 查询注册表 —— 列定义在代码,随版本发布走
 * ===========================================================================
 * 真实项目中抽到 src/queries/registry.ts:
 *   export const QUERY_REGISTRY = {
 *     "inv.credit_note":      { label: "红冲发票查询",   columns: [...] },
 *     "inv.overview":         { label: "综合查询",       columns: [...] },
 *     "inv.tax_transfer_out": { label: "进项税转出查询", columns: [...] },
 *   } as const;
 *   export type QueryKey = keyof typeof QUERY_REGISTRY;
 * field 一旦入库即不可变;label 随时可改。
 * ======================================================================== */

const QUERY_REGISTRY = {
  "inv.credit_note": {
    label: "红冲发票查询",
    columns: [
      { field: "invoice_code",    label: "发票代码",    group: "发票主表",     width: 140, defaultVisible: true },
      { field: "invoice_no",      label: "发票号码",    group: "发票主表",     width: 120, defaultVisible: true },
      { field: "invoice_date",    label: "开票日期",    group: "发票主表",     width: 110, defaultVisible: true },
      { field: "vendor_name",     label: "供应商名称",  group: "客商主数据",   width: 220, defaultVisible: true },
      { field: "amount_net",      label: "金额 (无税)", group: "发票金额明细", width: 130, defaultVisible: true },
      { field: "tax_amount",      label: "税额",        group: "发票金额明细", width: 110, defaultVisible: true },
      { field: "tax_out",         label: "进项转出",    group: "税务调整表",   width: 110, defaultVisible: true },
      { field: "tax_out_reason",  label: "转出原因",    group: "税务调整表",   width: 180, defaultVisible: true },
      { field: "stamp_tax",       label: "印花税",      group: "税务调整表",   width: 100, defaultVisible: true },
      { field: "manual_override", label: "人工覆写",    group: "系统控制表",   width: 100, defaultVisible: true },
      { field: "ap_doc_no",       label: "AP凭证号",    group: "凭证关联表",   width: 140, defaultVisible: false },
      { field: "po_no",           label: "采购订单号",  group: "凭证关联表",   width: 140, defaultVisible: false },
      { field: "tax_rate",        label: "税率",        group: "发票金额明细", width: 90,  defaultVisible: false },
      { field: "amount_gross",    label: "价税合计",    group: "发票金额明细", width: 130, defaultVisible: false },
      { field: "check_code",      label: "校验码",      group: "发票主表",     width: 160, defaultVisible: false },
      { field: "machine_no",      label: "机器编号",    group: "发票主表",     width: 140, defaultVisible: false },
      { field: "vendor_tax_id",   label: "供应商税号",  group: "客商主数据",   width: 170, defaultVisible: false },
      { field: "ic_party",        label: "IC 关联方",   group: "客商主数据",   width: 140, defaultVisible: false },
    ],
  },
};

const MAX_HISTORY = 50;
const MAX_FROZEN = 4;
const CLICK_SUPPRESS_MS = 150;
const DND_MIME = "application/x-layout-field";

/* ===========================================================================
 * 2. 纯函数层 —— 不依赖 React,可直接单元测试
 * ======================================================================== */

const idxOf = (cfg, field) => cfg.columns.findIndex((c) => c.field === field);

/** 固定:取出后插到冻结区末尾,frozen + 1 */
function pin(cfg, field) {
  const i = idxOf(cfg, field);
  if (i === -1 || i < cfg.frozen || cfg.frozen >= MAX_FROZEN) return cfg;
  const next = [...cfg.columns];
  const [moved] = next.splice(i, 1);
  next.splice(cfg.frozen, 0, moved);
  return { ...cfg, columns: next, frozen: cfg.frozen + 1 };
}

/** 取消固定:移到冻结区之后第一个位置,frozen - 1 */
function unpin(cfg, field) {
  const i = idxOf(cfg, field);
  if (i === -1 || i >= cfg.frozen) return cfg;
  const next = [...cfg.columns];
  const [moved] = next.splice(i, 1);
  next.splice(cfg.frozen - 1, 0, moved);
  return { ...cfg, columns: next, frozen: cfg.frozen - 1 };
}

/** 移动:跨区拖拽自动改变冻结状态,比「禁止跨区」更符合直觉 */
function move(cfg, from, to) {
  if (from === to || from < 0 || to < 0 || from >= cfg.columns.length) return cfg;
  const next = [...cfg.columns];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  let frozen = cfg.frozen;
  if (from >= cfg.frozen && to < cfg.frozen) frozen = Math.min(cfg.frozen + 1, MAX_FROZEN);
  else if (from < cfg.frozen && to >= cfg.frozen) frozen = cfg.frozen - 1;
  return { ...cfg, columns: next, frozen };
}

function addField(cfg, field, def) {
  if (idxOf(cfg, field) !== -1) return cfg;
  return { ...cfg, columns: [...cfg.columns, { field, width: def?.width }] };
}

function removeField(cfg, field) {
  const i = idxOf(cfg, field);
  if (i === -1) return cfg;
  return {
    ...cfg,
    columns: cfg.columns.filter((c) => c.field !== field),
    frozen: i < cfg.frozen ? cfg.frozen - 1 : cfg.frozen,
  };
}

/** 从注册表生成出厂默认 config —— SYSTEM layout 的内容来源 */
function defaultConfig(queryKey) {
  const defs = QUERY_REGISTRY[queryKey].columns;
  return {
    v: 2,
    columns: defs.filter((d) => d.defaultVisible).map((d) => ({ field: d.field, width: d.width })),
    frozen: 2,
    sort: [],
  };
}

/** v1 → v2 迁移。必须保留:同事的 db 副本里可能还是 v1 结构 */
function migrate(raw, version) {
  const cfg = typeof raw === "string" ? JSON.parse(raw) : raw;
  if (version >= 2) return cfg;
  const visible = (cfg.columns ?? []).filter((c) => c.visible).sort((a, b) => a.order - b.order);
  const pinned = visible.filter((c) => c.pinned);
  const rest = visible.filter((c) => !c.pinned);
  return {
    v: 2,
    columns: [...pinned, ...rest].map((c) => ({ field: c.field, width: c.width })),
    frozen: pinned.length,
    sort: cfg.sort ?? [],
  };
}

/**
 * 字段容错:以注册表为准对齐。
 * 已删除/改名的字段静默丢弃,注册表新增字段按 defaultVisible 追加。
 * 绝不报错或清空 —— 老同事存了半年的 layout 不能因为加了一列就失效。
 */
function resolveLayout(queryKey, cfg) {
  const defs = QUERY_REGISTRY[queryKey].columns;
  const known = new Map(defs.map((d) => [d.field, d]));
  const kept = cfg.columns.filter((c) => known.has(c.field));
  const droppedBefore = cfg.columns.slice(0, cfg.frozen).filter((c) => !known.has(c.field)).length;
  const seen = new Set(kept.map((c) => c.field));
  const added = defs
    .filter((d) => !seen.has(d.field) && d.defaultVisible)
    .map((d) => ({ field: d.field, width: d.width }));
  return {
    ...cfg,
    v: 2,
    columns: [...kept, ...added],
    frozen: Math.max(0, cfg.frozen - droppedBefore),
  };
}

const sameConfig = (a, b) => JSON.stringify(a) === JSON.stringify(b);

/* ===========================================================================
 * 3. 轻量 UI 原子 —— 真实项目中从 ./liquid-ui 导入
 * ======================================================================== */

function LiquidInput({ icon: Icon, clearable, onClear, ...props }) {
  return (
    <div className="relative">
      {Icon && (
        <Icon className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400" />
      )}
      <input
        {...props}
        className="w-full rounded-xl bg-white py-2 pl-8 pr-8 text-[12px] text-slate-800 ring-1 ring-slate-200 transition placeholder:text-slate-400 focus:outline-none focus-visible:ring-2 focus-visible:ring-sky-500 dark:bg-slate-800 dark:text-slate-100 dark:ring-slate-700"
      />
      {clearable && props.value ? (
        <button
          type="button"
          onClick={onClear}
          aria-label="清空搜索"
          className="absolute right-2 top-1/2 -translate-y-1/2 rounded-md p-0.5 text-slate-400 transition hover:text-slate-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sky-500 dark:hover:text-slate-200"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      ) : null}
    </div>
  );
}

const BTN = {
  primary:
    "bg-sky-600 text-white ring-1 ring-sky-600 hover:bg-sky-700 disabled:bg-slate-300 disabled:ring-slate-300 dark:disabled:bg-slate-700 dark:disabled:ring-slate-700",
  secondary:
    "bg-white text-slate-700 ring-1 ring-slate-200 hover:bg-slate-50 dark:bg-slate-800 dark:text-slate-200 dark:ring-slate-700 dark:hover:bg-slate-700",
  ghost:
    "bg-transparent text-slate-500 ring-1 ring-transparent hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-slate-800",
};

function LiquidButton({ variant = "secondary", icon: Icon, children, className = "", ...props }) {
  return (
    <button
      type="button"
      {...props}
      className={`inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-[12px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sky-500 focus-visible:ring-offset-1 disabled:cursor-not-allowed disabled:opacity-50 ${BTN[variant]} ${className}`}
    >
      {Icon && <Icon className="h-3.5 w-3.5" weight="bold" />}
      {children}
    </button>
  );
}

/* ===========================================================================
 * 4. Row —— 必须在模块顶层!
 * ===========================================================================
 * 若定义在 LayoutConfigurator 内部,dragover 期间的每次 setState 都会创建
 * 新的组件类型,React 卸载重建整棵子树,被拖拽的 DOM 节点消失,
 * 浏览器立即中止拖拽 —— 这是 HTML5 DnD 最常见的失效原因。
 * ======================================================================== */

const Row = memo(function Row({
  def,
  index,
  kind,            // "pinned" | "active" | "pool"
  dragging,
  insertBefore,
  insertAfter,
  frozenFull,
  onRowClick,
  onTogglePin,
  onDragStart,
  onDragOverRow,
  onDropRow,
  onDragEnd,
}) {
  const pool = kind === "pool";
  const pinned = kind === "pinned";

  const title = pool
    ? `添加「${def.label}」到活动列`
    : pinned
      ? `点击取消固定「${def.label}」`
      : `点击移除「${def.label}」`;

  return (
    <div className="relative">
      {insertBefore && (
        <div className="pointer-events-none absolute -top-1 left-0 right-0 z-10 h-0.5 rounded-full bg-sky-500" />
      )}
      {insertAfter && (
        <div className="pointer-events-none absolute -bottom-1 left-0 right-0 z-10 h-0.5 rounded-full bg-sky-500" />
      )}
      <div
        title={title}
        draggable={!pool}
        onDragStart={pool ? undefined : (e) => onDragStart(e, index)}
        onDragOver={pool ? undefined : (e) => onDragOverRow(e, index)}
        onDrop={pool ? undefined : onDropRow}
        onDragEnd={pool ? undefined : onDragEnd}
        onClick={() => onRowClick(def.field, kind)}
        className={`flex select-none items-center justify-between rounded-xl p-2 transition ${
          pool
            ? "cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-800/40"
            : "cursor-grab bg-white shadow-sm ring-1 ring-slate-200 hover:bg-slate-50 active:cursor-grabbing dark:bg-slate-800/60 dark:ring-slate-700 dark:hover:bg-slate-800"
        } ${dragging ? "opacity-60 ring-2 ring-sky-400 ring-offset-1" : ""}`}
      >
        <div className="flex min-w-0 items-center gap-2.5">
          {pool ? (
            <ArrowRight className="h-3.5 w-3.5 shrink-0 text-slate-300 dark:text-slate-600" />
          ) : (
            <DotsSixVertical className="h-4 w-4 shrink-0 text-slate-400" />
          )}
          <span
            className={`h-2 w-2 shrink-0 rounded-full ${
              pool
                ? "bg-slate-300 dark:bg-slate-600"
                : "bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.8)]"
            }`}
          />
          <span
            className={`truncate text-[12px] font-medium ${
              pool ? "text-slate-500 dark:text-slate-400" : "text-slate-800 dark:text-slate-200"
            }`}
          >
            {def.label}
          </span>
         
        </div>

        {!pool && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onTogglePin(def.field, pinned);
            }}
            disabled={!pinned && frozenFull}
            title={pinned ? "取消固定" : frozenFull ? `最多固定 ${MAX_FROZEN} 列` : "固定到左侧"}
            className={`shrink-0 rounded-lg p-1.5 transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sky-500 disabled:cursor-not-allowed disabled:opacity-40 ${
              pinned
                ? "bg-sky-50 text-sky-500 hover:bg-sky-100 dark:bg-sky-500/10 dark:hover:bg-sky-500/20"
                : "text-slate-400 hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-700"
            }`}
          >
            <PushPin weight={pinned ? "fill" : "regular"} className="h-3.5 w-3.5" />
          </button>
        )}
      </div>
    </div>
  );
});

/* ===========================================================================
 * 5. 主组件 —— 受控:字段来自注册表,当前值来自数据库
 * ======================================================================== */

export function LayoutConfigurator({
  isOpen,
  onClose,
  queryKey = "inv.credit_note",
  currentLayout,
  initialConfig,
  onApply,
  onSave,
  onSaveAs,
}) {
  const registry = QUERY_REGISTRY[queryKey];
  const baseline = useMemo(
    () => resolveLayout(queryKey, initialConfig ?? defaultConfig(queryKey)),
    [queryKey, initialConfig],
  );

  // 两段式撤销栈,避免 push/pop 混用产生「未来分支」
  const [hist, setHist] = useState({ past: [], present: baseline });
  const cfg = hist.present;

  const [search, setSearch] = useState("");
  const [collapsed, setCollapsed] = useState({});
  const [dragIdx, setDragIdx] = useState(null);
  const [overIdx, setOverIdx] = useState(null);
  const [saveAsOpen, setSaveAsOpen] = useState(false);
  const [saveAsName, setSaveAsName] = useState("");
  const [saveAsScope, setSaveAsScope] = useState("SHARED");
  const [toast, setToast] = useState(null);
  const [mounted, setMounted] = useState(isOpen);

  // 拖拽结束后浏览器会补发一次 click。行的单击是破坏性操作(移除/取消固定),
  // 所以拖完 150ms 内忽略 click。原地按下又松开同样走这条路径。
  const suppressClickUntil = useRef(0);
  const panelRef = useRef(null);
  const closeBtnRef = useRef(null);
  const cfgRef = useRef(cfg);
  cfgRef.current = cfg;
  // 拖拽状态的唯一真相(同步可读);state 仅用于渲染插入线
  const dragFrom = useRef(null);
  const dropTo = useRef(null);

  const isSystem = currentLayout?.scope === "SYSTEM";
  const dirty = !sameConfig(cfg, baseline);
  const canSave = cfg.columns.length > 0;
  const frozenFull = cfg.frozen >= MAX_FROZEN;

  useEffect(() => {
    if (isOpen) {
      setMounted(true);
      setHist({ past: [], present: baseline });
      setSearch("");
      setSaveAsOpen(false);
      const t = setTimeout(() => closeBtnRef.current?.focus(), 320);
      return () => clearTimeout(t);
    }
    const t = setTimeout(() => setMounted(false), 300);
    return () => clearTimeout(t);
  }, [isOpen, baseline]);

  const flash = useCallback((msg) => {
    setToast(msg);
    setTimeout(() => setToast(null), 1800);
  }, []);

  const commit = useCallback((next) => {
    setHist((h) => {
      if (sameConfig(next, h.present)) return h; // 无变化不入栈
      return { past: [...h.past, h.present].slice(-MAX_HISTORY), present: next };
    });
  }, []);

  const undo = useCallback(() => {
    setHist((h) =>
      h.past.length ? { past: h.past.slice(0, -1), present: h.past[h.past.length - 1] } : h,
    );
  }, []);

  const requestClose = useCallback(() => {
    if (dirty && !window.confirm("有未保存的改动,关闭后会丢失。确定关闭?")) return;
    onClose?.();
  }, [dirty, onClose]);

  // 只处理 Esc。不拦 Tab、不拦方向键 —— 方向键留给列表滚动。
  useEffect(() => {
    if (!isOpen) return;
    const onKey = (e) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      if (saveAsOpen) setSaveAsOpen(false);
      else requestClose();
    };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, [isOpen, saveAsOpen, requestClose]);

  /* ---------- 稳定的回调:让 memo 过的 Row 不白白重渲染 ---------- */

  const handleRowClick = useCallback(
    (field, kind) => {
      if (Date.now() < suppressClickUntil.current) return;
      const c = cfgRef.current;
      // 单击语义按原设计:备选=添加, 活动=移除, 冻结=取消固定
      if (kind === "pool") {
        commit(addField(c, field, registry.columns.find((d) => d.field === field)));
      } else if (kind === "pinned") {
        commit(unpin(c, field));
      } else {
        commit(removeField(c, field));
      }
    },
    [commit, registry],
  );

  const handleTogglePin = useCallback(
    (field, isPinned) => {
      const c = cfgRef.current;
      commit(isPinned ? unpin(c, field) : pin(c, field));
    },
    [commit],
  );

  /* ---------- 拖拽 ----------
   * 拖拽状态的唯一真相是 ref,不是 state。
   * 原因:drop/dragend 回调里需要立刻读到最新的 from/to 并算出结果,
   * 而 state 更新是异步的。此前把 setOverIdx 嵌套写在 setDragIdx 的
   * updater 里 —— updater 必须是纯函数,React 18 会丢弃这种嵌套调用,
   * move() 根本不会执行,于是「松手后什么都没变」。
   * state 只用来驱动插入线的渲染。
   */
  const handleDragStart = useCallback((e, idx) => {
    // WebView2 / WebKit 下不 setData 可能根本不启动拖拽
    e.dataTransfer.setData(DND_MIME, String(idx));
    e.dataTransfer.setData("text/plain", String(idx));
    e.dataTransfer.effectAllowed = "move";
    dragFrom.current = idx;
    dropTo.current = idx;
    setDragIdx(idx);
    setOverIdx(idx);
  }, []);

  // dragover 只记录落点,不动数据
  const handleDragOverRow = useCallback((e, idx) => {
    if (dragFrom.current === null) return;
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = "move";
    const r = e.currentTarget.getBoundingClientRect();
    const next = e.clientY > r.top + r.height / 2 ? idx + 1 : idx;
    if (dropTo.current !== next) {
      dropTo.current = next;
      setOverIdx(next);
    }
  }, []);

  // drop / dragend 时才真正移动一次
  const finishDrag = useCallback(() => {
    const from = dragFrom.current;
    const to = dropTo.current;
    dragFrom.current = null;
    dropTo.current = null;
    setDragIdx(null);
    setOverIdx(null);
    suppressClickUntil.current = Date.now() + CLICK_SUPPRESS_MS;
    if (from === null || to === null) return;
    commit(move(cfgRef.current, from, to > from ? to - 1 : to));
  }, [commit]);

  const handleDropRow = useCallback(
    (e) => {
      e.preventDefault();
      e.stopPropagation();
      finishDrag();
    },
    [finishDrag],
  );

  // 落在区块空白处:插到该区块末尾
  const containerDragOver = useCallback((e, endIdx) => {
    if (dragFrom.current === null) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    if (e.target !== e.currentTarget) return; // 行自己处理过了
    if (dropTo.current !== endIdx) {
      dropTo.current = endIdx;
      setOverIdx(endIdx);
    }
  }, []);

  if (!mounted) return null;

  /* ---------- 派生数据 ---------- */
  const defByField = new Map(registry.columns.map((d) => [d.field, d]));
  const selected = cfg.columns.map((c, i) => ({
    def: defByField.get(c.field),
    width: c.width,
    field: c.field,
    index: i,
    pinned: i < cfg.frozen,
  }));

  const q = search.trim().toLowerCase();
  const matches = (d) => !q || d.label.toLowerCase().includes(q) || d.group.toLowerCase().includes(q);
  const inLayout = new Set(cfg.columns.map((c) => c.field));
  const poolDefs = registry.columns.filter((d) => !inLayout.has(d.field) && matches(d));
  const alreadyUsed = q ? registry.columns.filter((d) => inLayout.has(d.field) && matches(d)) : [];

  const groups = poolDefs.reduce((acc, d) => {
    (acc[d.group] ??= []).push(d);
    return acc;
  }, {});

  const rowProps = (r, kind) => ({
    key: r.field,
    def: r.def,
    index: r.index,
    kind,
    dragging: dragIdx === r.index,
    // 插入线:落点在本行之前,或落点在列表末尾且本行是最后一行
    insertBefore: dragIdx !== null && overIdx === r.index,
    insertAfter: dragIdx !== null && overIdx === cfg.columns.length && r.index === cfg.columns.length - 1,
    frozenFull,
    onRowClick: handleRowClick,
    onTogglePin: handleTogglePin,
    onDragStart: handleDragStart,
    onDragOverRow: handleDragOverRow,
    onDropRow: handleDropRow,
    onDragEnd: finishDrag,
  });

  return (
    <>
      <div
        className={`fixed inset-0 z-40 bg-slate-900/20 backdrop-blur-sm transition-opacity duration-300 ${
          isOpen ? "opacity-100" : "pointer-events-none opacity-0"
        }`}
        onClick={requestClose}
      />

      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label="表格布局配置"
        style={{ "--chrome-h": "62px" }}
        className={`fixed right-2 top-[var(--chrome-h)] bottom-[var(--chrome-h)] z-50 flex w-[400px] max-w-[calc(100vw-1rem)] flex-col overflow-hidden rounded-3xl bg-white shadow-[-10px_0_40px_rgba(0,0,0,0.12)] ring-1 ring-slate-200 transition-transform duration-300 ease-[cubic-bezier(0.16,1,0.3,1)] dark:bg-slate-900 dark:ring-white/10 ${
          isOpen ? "translate-x-0" : "translate-x-[110%]"
        }`}
      >
        {/* 头部 */}
        <div className="flex items-start justify-between gap-2 border-b border-slate-200 p-4 dark:border-slate-800">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-slate-800 dark:text-slate-100">
              <TableIcon weight="duotone" className="h-5 w-5 shrink-0 text-sky-500" />
              <h2 className="truncate text-sm font-bold">布局配置 · {registry.label}</h2>
            </div>
            <div className="mt-1 flex items-center gap-1.5 pl-7 text-[11px] text-slate-400">
              <span className="truncate">{currentLayout?.name ?? "未命名"}</span>
              {isSystem && (
                <span className="inline-flex shrink-0 items-center gap-1 rounded-md bg-slate-100 px-1.5 py-0.5 text-slate-500 dark:bg-slate-800">
                  <Lock className="h-3 w-3" />
                  系统默认
                </span>
              )}
              {dirty && <span className="shrink-0 text-amber-500">· 已修改</span>}
            </div>
          </div>
          <button
            ref={closeBtnRef}
            type="button"
            onClick={requestClose}
            aria-label="关闭"
            className="rounded-full p-1.5 text-slate-400 transition hover:bg-slate-100 hover:text-slate-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sky-500 dark:hover:bg-slate-800 dark:hover:text-slate-200"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* 实时预览条 */}
        <div className="flex items-center gap-2 border-b border-slate-200 bg-slate-50 px-4 py-2.5 dark:border-slate-800 dark:bg-slate-900/60">
          <span className="shrink-0 text-[10px] font-semibold uppercase tracking-wide text-slate-400">
            预览
          </span>
          <div className="flex min-w-0 flex-1 items-center gap-1 overflow-hidden">
            {selected.length === 0 && <span className="text-[11px] text-slate-400">未选择任何列</span>}
            {selected.map((r) => (
              <span key={r.field} className="flex items-center gap-1">
                {r.index === cfg.frozen && cfg.frozen > 0 && (
                  <span className="mx-0.5 h-3.5 w-px shrink-0 bg-sky-400" />
                )}
                <span
                  className={`h-1.5 shrink-0 rounded-full ${
                    r.pinned ? "bg-sky-500" : "bg-slate-300 dark:bg-slate-600"
                  }`}
                  style={{ width: Math.max(8, Math.min(28, (r.width ?? 100) / 6)) }}
                />
              </span>
            ))}
          </div>
          <span className="shrink-0 text-[10px] tabular-nums text-slate-400">
            {selected.length} 列
          </span>
        </div>

        {/* 内容区 */}
        <div className="flex-1 space-y-5 overflow-y-auto p-4">
          {cfg.frozen > 0 && (
            <section className="space-y-2">
              <div className="flex items-center justify-between px-1 text-[11px] font-bold text-sky-600 dark:text-sky-400">
                <span className="flex items-center gap-1.5">
                  <PushPin weight="fill" className="h-3.5 w-3.5" />
                  冻结列 ({cfg.frozen}/{MAX_FROZEN})
                </span>
                <span className="font-normal text-slate-400">点击行取消固定</span>
              </div>
              <div
                className="space-y-1.5 rounded-2xl bg-sky-50/60 p-1.5 ring-1 ring-sky-100 dark:bg-sky-500/5 dark:ring-sky-500/10"
                onDragOver={(e) => containerDragOver(e, cfg.frozen)}
                onDrop={handleDropRow}
              >
                {selected.slice(0, cfg.frozen).map((r) => (
                  <Row {...rowProps(r, "pinned")} />
                ))}
              </div>
            </section>
          )}

          <section className="space-y-2">
            <div className="flex items-center justify-between px-1 text-[11px] font-bold text-slate-600 dark:text-slate-400">
              <span>活动列 ({cfg.columns.length - cfg.frozen})</span>
              <span className="font-normal text-slate-400">拖拽排序</span>
            </div>
            <div
              className="min-h-[56px] space-y-1.5 rounded-2xl bg-slate-50 p-1.5 ring-1 ring-slate-200 dark:bg-slate-800/40 dark:ring-slate-700/60"
              onDragOver={(e) => containerDragOver(e, cfg.columns.length)}
              onDrop={handleDropRow}
            >
              {cfg.columns.length - cfg.frozen === 0 ? (
                <p className="px-2 py-3 text-center text-[11px] text-slate-400">
                  从下方字段库点击添加,或把冻结列拖到这里
                </p>
              ) : (
                selected.slice(cfg.frozen).map((r) => <Row {...rowProps(r, "active")} />)
              )}
            </div>
          </section>

          <hr className="border-slate-200 dark:border-slate-700/60" />

          <section className="space-y-3 pb-4">
            <span className="block px-1 text-[11px] font-bold text-slate-600 dark:text-slate-400">
              备选字段库
            </span>

            <LiquidInput
              icon={MagnifyingGlass}
              placeholder="搜索字段名或源表…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              clearable
              onClear={() => setSearch("")}
            />

            {alreadyUsed.length > 0 && (
              <div className="space-y-1 rounded-xl bg-emerald-50 px-2.5 py-2 text-[11px] text-emerald-700 ring-1 ring-emerald-100 dark:bg-emerald-500/10 dark:text-emerald-300 dark:ring-emerald-500/20">
                {alreadyUsed.slice(0, 4).map((d) => (
                  <div key={d.field} className="flex items-center gap-1.5">
                    <CheckCircle weight="fill" className="h-3.5 w-3.5 shrink-0" />
                    <span>
                      「{d.label}」已在
                      {idxOf(cfg, d.field) < cfg.frozen ? "冻结列" : "活动列"}中
                    </span>
                  </div>
                ))}
              </div>
            )}

            <div className="space-y-2">
              {Object.entries(groups).map(([group, defs]) => {
                const open = !collapsed[group];
                return (
                  <div
                    key={group}
                    className="overflow-hidden rounded-xl ring-1 ring-slate-200 dark:ring-slate-700/60"
                  >
                    <button
                      type="button"
                      onClick={() => setCollapsed((p) => ({ ...p, [group]: !p[group] }))}
                      aria-expanded={open}
                      className="flex w-full items-center justify-between bg-slate-50 p-2.5 transition hover:bg-slate-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-sky-500 dark:bg-slate-800/50 dark:hover:bg-slate-800"
                    >
                      <span className="text-[11px] font-semibold text-slate-700 dark:text-slate-300">
                        {group}
                      </span>
                      <span className="flex items-center gap-2">
                        <span className="rounded-md bg-white px-1.5 py-0.5 text-[10px] tabular-nums text-slate-400 dark:bg-slate-900">
                          {defs.length}
                        </span>
                        {open ? (
                          <CaretUp className="h-3.5 w-3.5 text-slate-400" />
                        ) : (
                          <CaretDown className="h-3.5 w-3.5 text-slate-400" />
                        )}
                      </span>
                    </button>
                    {open && (
                      <div className="space-y-0.5 border-t border-slate-200 p-1.5 dark:border-slate-700/60">
                        {defs.map((d) => (
                          <Row
                            key={d.field}
                            def={d}
                            index={-1}
                            kind="pool"
                            dragging={false}
                            insertBefore={false}
                            insertAfter={false}
                            frozenFull={frozenFull}
                            onRowClick={handleRowClick}
                            onTogglePin={handleTogglePin}
                          />
                        ))}
                      </div>
                    )}
                  </div>
                );
              })}
              {Object.keys(groups).length === 0 && (
                <p className="py-6 text-center text-[11px] text-slate-400">
                  {q ? "没有匹配的备选字段" : "所有字段都已加入布局"}
                </p>
              )}
            </div>
          </section>
        </div>

        {/* 底部操作栏 */}
        <div className="border-t border-slate-200 bg-white p-3 dark:border-slate-800 dark:bg-slate-900">
          {!canSave && (
            <p className="mb-2 px-1 text-[11px] text-amber-600 dark:text-amber-400">
              至少保留一列才能保存或应用。
            </p>
          )}
          <div className="flex items-center justify-between gap-2">
            <LiquidButton
              variant="ghost"
              icon={ArrowUUpLeft}
              onClick={undo}
              disabled={hist.past.length === 0}
            >
              撤销{hist.past.length > 0 ? ` (${hist.past.length})` : ""}
            </LiquidButton>

            <div className="flex items-center gap-1.5">
              <LiquidButton
                variant="secondary"
                onClick={() => {
                  setSaveAsName("");
                  setSaveAsOpen(true);
                }}
                disabled={!canSave}
              >
                另存为…
              </LiquidButton>
              <LiquidButton
                variant="secondary"
                icon={FloppyDisk}
                onClick={async () => {
                  await onSave?.(currentLayout?.id, cfg);
                  flash("已保存");
                  setHist({ past: [], present: cfg });
                }}
                disabled={!canSave || isSystem}
                title={isSystem ? "系统默认布局不可修改,请使用「另存为」" : "覆盖当前布局"}
              >
                保存
              </LiquidButton>
              <LiquidButton
                variant="primary"
                icon={Play}
                onClick={() => {
                  onApply?.(cfg);
                  onClose?.();
                }}
                disabled={!canSave}
                title="仅应用到当前结果,不写入数据库"
              >
                应用
              </LiquidButton>
            </div>
          </div>
        </div>

        {/* 另存为 */}
        {saveAsOpen && (
          <div className="absolute inset-0 z-20 flex items-end bg-slate-900/30 backdrop-blur-sm">
            <div className="w-full space-y-3 rounded-b-3xl border-t border-slate-200 bg-white p-4 dark:border-slate-700 dark:bg-slate-900">
              <h3 className="text-[12px] font-bold text-slate-800 dark:text-slate-100">
                另存为新布局
              </h3>
              <LiquidInput
                autoFocus
                placeholder="例如 Phil_1618_红冲"
                value={saveAsName}
                onChange={(e) => setSaveAsName(e.target.value)}
              />
              <div className="flex gap-1.5">
                {[
                  { v: "SHARED", label: "共享 · 本 LE 所有人可见" },
                  { v: "PRIVATE", label: "私有 · 仅自己可见" },
                ].map((o) => (
                  <button
                    key={o.v}
                    type="button"
                    onClick={() => setSaveAsScope(o.v)}
                    aria-pressed={saveAsScope === o.v}
                    className={`flex-1 rounded-xl px-2 py-2 text-[11px] font-medium transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sky-500 ${
                      saveAsScope === o.v
                        ? "bg-sky-50 text-sky-700 ring-1 ring-sky-300 dark:bg-sky-500/10 dark:text-sky-300 dark:ring-sky-500/30"
                        : "bg-slate-50 text-slate-500 ring-1 ring-slate-200 dark:bg-slate-800 dark:ring-slate-700"
                    }`}
                  >
                    {o.label}
                  </button>
                ))}
              </div>
              <div className="flex justify-end gap-2 pt-1">
                <LiquidButton variant="ghost" onClick={() => setSaveAsOpen(false)}>
                  取消
                </LiquidButton>
                <LiquidButton
                  variant="primary"
                  icon={FloppyDisk}
                  disabled={!saveAsName.trim()}
                  onClick={async () => {
                    await onSaveAs?.({ name: saveAsName.trim(), scope: saveAsScope, cfg });
                    setSaveAsOpen(false);
                    flash(`已另存为「${saveAsName.trim()}」`);
                    setHist({ past: [], present: cfg });
                  }}
                >
                  保存
                </LiquidButton>
              </div>
            </div>
          </div>
        )}

        {toast && (
          <div
            role="status"
            className="pointer-events-none absolute bottom-20 left-1/2 -translate-x-1/2 rounded-xl bg-slate-900 px-3 py-1.5 text-[11px] font-medium text-white shadow-lg dark:bg-slate-700"
          >
            {toast}
          </div>
        )}
      </div>
    </>
  );
}

/* ===========================================================================
 * 6. 真实场景接线 (全部注释,当前用模拟数据)
 * ===========================================================================
 *
 * ---------- 6.1 Rust 侧命令 (src-tauri/src/layout.rs) ----------
 *
 * #[tauri::command]
 * pub fn list_layouts(db: State<DbPool>, query_key: String, user: String)
 *     -> Result<Vec<LayoutMeta>, String>
 * {
 *     let conn = db.current_le()?;          // 当前切换到的 LE 库,如 1618.db
 *     let mut stmt = conn.prepare(
 *         "SELECT id, name, scope, owner, is_global_default,
 *                 config_json, config_version
 *            FROM query_layout
 *           WHERE query_key = ?1
 *             AND is_deleted = 0
 *             AND (scope IN ('SYSTEM','SHARED') OR owner = ?2)
 *           ORDER BY scope = 'SYSTEM' DESC, name",
 *     )?;
 *     // 同事做 1618 backup 时:能看到 SYSTEM 默认 + Phil 共享的布局,
 *     // 看不到 Phil 的私有草稿 —— 与 SAP 的用户专用/全局变式语义一致
 *     ...
 * }
 *
 * #[tauri::command]
 * pub fn save_layout(db: State<DbPool>, id: String, config_json: String, user: String)
 *     -> Result<(), String>
 * {
 *     db.current_le()?.execute(
 *         "UPDATE query_layout
 *             SET config_json = ?2, config_version = 2, updated_by = ?3
 *           WHERE id = ?1 AND scope <> 'SYSTEM'",   // SYSTEM 布局不可改
 *         params![id, config_json, user],
 *     )?;
 *     Ok(())
 * }
 *
 * #[tauri::command]
 * pub fn create_layout(
 *     db: State<DbPool>, query_key: String, name: String,
 *     scope: String, config_json: String, user: String,
 * ) -> Result<String, String> {
 *     let id = Uuid::new_v4().to_string();     // UUID:不同副本上新建不会撞号
 *     db.current_le()?.execute(
 *         "INSERT INTO query_layout
 *            (id, query_key, name, scope, owner, config_json,
 *             config_version, created_by, updated_by)
 *          VALUES (?1,?2,?3,?4,?5,?6,2,?5,?5)",
 *         params![id, query_key, name, scope, user, config_json],
 *     )?;
 *     Ok(id)
 * }
 *
 * // 启动时按注册表幂等补齐 SYSTEM 布局(id 固定为 'sys:<query_key>'),
 * // 新增查询后老库下次启动自动获得默认布局:
 * //   INSERT INTO query_layout (...) VALUES (...) ON CONFLICT(id) DO NOTHING;
 *
 * ---------- 6.2 前端数据层 (TanStack Query) ----------
 *
 * import { invoke } from "@tauri-apps/api/core";
 * import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
 *
 * export function useLayouts(queryKey: QueryKey) {
 *   const { data: user } = useCurrentUser();
 *   const { data: le }   = useCurrentLe();   // LE 切换后 key 变化 → 自动重取
 *
 *   return useQuery({
 *     queryKey: ["layouts", le, queryKey],
 *     queryFn: async () => {
 *       const rows = await invoke<LayoutRow[]>("list_layouts", { queryKey, user: user.id });
 *       // 读取时迁移 + 按注册表对齐,双重容错
 *       return rows.map((r) => ({
 *         ...r,
 *         config: resolveLayout(queryKey, migrate(r.config_json, r.config_version)),
 *       }));
 *     },
 *   });
 * }
 *
 * ---------- 6.3 页面接线 ----------
 *
 * function CreditNotePage() {
 *   const QK = "inv.credit_note" as const;
 *   const qc = useQueryClient();
 *   const { data: layouts = [] } = useLayouts(QK);
 *   const [layoutId, setLayoutId] = useState<string | null>(null);
 *   const [override, setOverride] = useState<LayoutConfig | null>(null);  // 临时「应用」
 *   const [open, setOpen] = useState(false);
 *   const [filters, setFilters] = useState<Filters>({});
 *
 *   // 默认布局解析顺序:用户偏好 → is_global_default → SYSTEM
 *   const current =
 *     layouts.find((l) => l.id === layoutId) ??
 *     layouts.find((l) => l.is_global_default) ??
 *     layouts.find((l) => l.scope === "SYSTEM")!;
 *   const cfg = override ?? current.config;
 *
 *   // 关键:layoutId / cfg 不进 queryKey —— 切布局是纯前端操作,不打数据库
 *   const { data: rows = [] } = useQuery({
 *     queryKey: [QK, filters],
 *     queryFn: () => invoke("query_credit_notes", { filters }),
 *   });
 *
 *   // SQL 回答「取哪些行」,layout 回答「怎么展示这些行」
 *   const columns = useMemo(() => {
 *     const defs = new Map(QUERY_REGISTRY[QK].columns.map((d) => [d.field, d]));
 *     return cfg.columns.map((c) => {
 *       const d = defs.get(c.field)!;
 *       return columnHelper.accessor(c.field, { header: d.label, size: c.width ?? d.width });
 *     });
 *   }, [cfg]);
 *
 *   const table = useReactTable({
 *     data: rows,
 *     columns,
 *     getCoreRowModel: getCoreRowModel(),
 *     // 冻结列直接复用数组前缀,无需额外状态
 *     state: { columnPinning: { left: cfg.columns.slice(0, cfg.frozen).map((c) => c.field) } },
 *   });
 *
 *   const saveMut = useMutation({
 *     mutationFn: ({ id, cfg }) => invoke("save_layout", { id, configJson: JSON.stringify(cfg) }),
 *     onSuccess: () => qc.invalidateQueries({ queryKey: ["layouts"] }),
 *   });
 *
 *   const saveAsMut = useMutation({
 *     mutationFn: ({ name, scope, cfg }) =>
 *       invoke<string>("create_layout", {
 *         queryKey: QK, name, scope, configJson: JSON.stringify(cfg),
 *       }),
 *     onSuccess: (newId) => {
 *       qc.invalidateQueries({ queryKey: ["layouts"] });
 *       setLayoutId(newId);
 *       setOverride(null);
 *     },
 *   });
 *
 *   return (
 *     <>
 *       <LayoutPicker layouts={layouts} value={current.id} onChange={(id) => {
 *         setLayoutId(id);
 *         setOverride(null);
 *         invoke("set_layout_preference", { queryKey: QK, layoutId: id });  // 记住上次使用
 *       }} />
 *
 *       <VirtualizedTable table={table} />   {/* 不分页:TanStack Virtual 全量渲染 *\/}
 *
 *       {/* 导出只传「导出什么」,不传数据 —— 避免大 payload 跨 IPC 两趟 *\/}
 *       <button onClick={async () => {
 *         const path = await save({ defaultPath: "红冲发票_1618.xlsx" });
 *         if (path) await invoke("export_credit_notes", {
 *           filters, path,
 *           columns: cfg.columns.map((c) => c.field),   // 按当前布局导出
 *         });
 *       }}>导出 Excel</button>
 *
 *       <LayoutConfigurator
 *         isOpen={open}
 *         onClose={() => setOpen(false)}
 *         queryKey={QK}
 *         currentLayout={current}
 *         initialConfig={cfg}
 *         onApply={setOverride}
 *         onSave={(id, cfg) => saveMut.mutateAsync({ id, cfg })}
 *         onSaveAs={(input) => saveAsMut.mutateAsync(input)}
 *       />
 *     </>
 *   );
 * }
 * ======================================================================== */

/* ===========================================================================
 * 7. 演示外壳 —— 模拟数据,替代上面注释掉的真实数据层
 * ======================================================================== */

const MOCK_LAYOUTS = [
  { id: "sys:inv.credit_note", name: "SYSTEM_DEFAULT", scope: "SYSTEM", owner: "SYSTEM", is_global_default: 1 },
  { id: "l-2", name: "Phil_1618_红冲", scope: "SHARED", owner: "phil", is_global_default: 0 },
  { id: "l-3", name: "月度核对_简版", scope: "PRIVATE", owner: "phil", is_global_default: 0 },
];

const MOCK_CONFIGS = {
  "sys:inv.credit_note": defaultConfig("inv.credit_note"),
  "l-2": {
    v: 2,
    frozen: 3,
    sort: [],
    columns: [
      { field: "invoice_no", width: 120 },
      { field: "vendor_name", width: 220 },
      { field: "invoice_date", width: 110 },
      { field: "tax_amount", width: 110 },
      { field: "tax_out", width: 110 },
      { field: "tax_out_reason", width: 180 },
      { field: "ap_doc_no", width: 140 },
    ],
  },
  "l-3": {
    v: 2,
    frozen: 1,
    sort: [],
    columns: [
      { field: "invoice_no", width: 120 },
      { field: "amount_net", width: 130 },
      { field: "tax_amount", width: 110 },
    ],
  },
};

export default function App() {
  const [open, setOpen] = useState(true);
  const [layoutId, setLayoutId] = useState("l-2");
  const [configs, setConfigs] = useState(MOCK_CONFIGS);
  const [layouts, setLayouts] = useState(MOCK_LAYOUTS);
  const [override, setOverride] = useState(null);
  const [log, setLog] = useState([]);

  const current = layouts.find((l) => l.id === layoutId) ?? layouts[0];
  const cfg = override ?? configs[current.id];
  const resolved = resolveLayout("inv.credit_note", cfg);
  const defs = new Map(QUERY_REGISTRY["inv.credit_note"].columns.map((d) => [d.field, d]));

  const push = (m) => setLog((l) => [`${new Date().toLocaleTimeString()} ${m}`, ...l].slice(0, 6));

  return (
    <div className="min-h-screen bg-slate-100 p-4 font-sans dark:bg-slate-950">
      <div className="mb-3 flex flex-wrap items-center gap-2 rounded-2xl bg-white p-3 ring-1 ring-slate-200 dark:bg-slate-900 dark:ring-slate-800">
        <span className="text-[12px] font-bold text-slate-700 dark:text-slate-200">红冲发票查询</span>
        <select
          value={layoutId}
          onChange={(e) => {
            setLayoutId(e.target.value);
            setOverride(null);
          }}
          className="rounded-xl bg-slate-50 px-2 py-1.5 text-[12px] text-slate-700 ring-1 ring-slate-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sky-500 dark:bg-slate-800 dark:text-slate-200 dark:ring-slate-700"
        >
          {layouts.map((l) => (
            <option key={l.id} value={l.id}>
              {l.name}
              {l.scope === "SYSTEM" ? " (默认)" : l.scope === "PRIVATE" ? " (私有)" : ""}
            </option>
          ))}
        </select>
        {override && <span className="text-[11px] text-amber-500">临时应用中</span>}
        <LiquidButton variant="primary" icon={TableIcon} onClick={() => setOpen(true)}>
          布局
        </LiquidButton>
      </div>

      {/* 模拟表格表头:冻结区用竖线与滚动区分隔 */}
      <div className="overflow-x-auto rounded-2xl bg-white ring-1 ring-slate-200 dark:bg-slate-900 dark:ring-slate-800">
        <div className="flex min-w-max">
          {resolved.columns.map((c, i) => (
            <div
              key={c.field}
              style={{ width: c.width ?? defs.get(c.field)?.width ?? 120 }}
              className={`shrink-0 border-b border-slate-200 px-3 py-2.5 text-[11px] font-semibold dark:border-slate-800 ${
                i < resolved.frozen
                  ? "bg-sky-50 text-sky-800 dark:bg-sky-500/10 dark:text-sky-200"
                  : "text-slate-600 dark:text-slate-300"
              } ${i === resolved.frozen - 1 && resolved.frozen > 0 ? "border-r-2 border-r-sky-400" : ""}`}
            >
              {defs.get(c.field)?.label ?? c.field}
            </div>
          ))}
        </div>
        {[0, 1, 2].map((r) => (
          <div key={r} className="flex min-w-max">
            {resolved.columns.map((c, i) => (
              <div
                key={c.field}
                style={{ width: c.width ?? defs.get(c.field)?.width ?? 120 }}
                className={`shrink-0 border-b border-slate-100 px-3 py-2 dark:border-slate-800/60 ${
                  i < resolved.frozen ? "bg-sky-50/40 dark:bg-sky-500/5" : ""
                } ${i === resolved.frozen - 1 && resolved.frozen > 0 ? "border-r-2 border-r-sky-400" : ""}`}
              >
                <div className="h-2 rounded-full bg-slate-200 dark:bg-slate-700" />
              </div>
            ))}
          </div>
        ))}
      </div>

      {log.length > 0 && (
        <ul className="mt-3 space-y-0.5 rounded-2xl bg-white p-3 text-[11px] text-slate-500 ring-1 ring-slate-200 dark:bg-slate-900 dark:text-slate-400 dark:ring-slate-800">
          {log.map((l, i) => (
            <li key={i} className="font-mono">{l}</li>
          ))}
        </ul>
      )}

      <LayoutConfigurator
        isOpen={open}
        onClose={() => setOpen(false)}
        queryKey="inv.credit_note"
        currentLayout={current}
        initialConfig={cfg}
        onApply={(c) => {
          setOverride(c);
          push(`应用临时布局 (${c.columns.length} 列, 冻结 ${c.frozen})`);
        }}
        onSave={(id, c) => {
          setConfigs((p) => ({ ...p, [id]: c }));
          setOverride(null);
          push(`save_layout(${id})`);
        }}
        onSaveAs={({ name, scope, cfg: c }) => {
          const id = `l-${Date.now()}`;
          setLayouts((p) => [...p, { id, name, scope, owner: "phil", is_global_default: 0 }]);
          setConfigs((p) => ({ ...p, [id]: c }));
          setLayoutId(id);
          setOverride(null);
          push(`create_layout("${name}", ${scope}) -> ${id}`);
          return id;
        }}
      />
    </div>
  );
}
