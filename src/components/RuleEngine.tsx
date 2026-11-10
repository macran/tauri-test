import React, { useMemo } from 'react'
import { useForm } from '@tanstack/react-form'
import { useQuery } from '@tanstack/react-query'
import {
  useReactTable,
  getCoreRowModel,
  createColumnHelper,
  flexRender,
} from '@tanstack/react-table-v8'
import { z } from 'zod'
import {
  Plus,
  MagnifyingGlass as Search,
  FadersHorizontal as SlidersHorizontal,
  ArrowUp,
  ArrowDown,
  PencilSimple as Edit3,
  Trash as Trash2,
  CheckCircle as CheckCircle2,
  XCircle,
  Sparkle as Sparkles,
  Stack as Layers,
  X,
  Check,
  ShieldWarning as ShieldAlert,
  Tag,
  FileXls as FileSpreadsheet,
} from '@phosphor-icons/react'
import { LiquidCard, LiquidInput, LiquidSelect, LiquidButton } from './liquid-ui'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from './ui/select'
import { useRuleStore } from '../store/useRuleStore'

// ── Types ────────────────────────────────────────────────────────────────────
export type RuleCategory = 'vat' | 'stamp'

export interface RuleCondition {
  id: string
  field: string
  operator: 'contains' | 'equals' | 'not_equals' | 'greater_than' | 'in_list'
  value: string
}

export interface RuleAction {
  vatTransfer?: boolean
  vatReason?: string
  stampDutyApplies?: boolean
  stampRate?: number
  stampCategory?: string
}

export interface TaxRule {
  id: string
  code: string
  name: string
  category: RuleCategory
  priority: number
  enabled: boolean
  logicOperator: 'AND' | 'OR'
  conditions: RuleCondition[]
  action: RuleAction
  leAppliesTo: string[]
  updatedBy: string
  updatedAt: string
}

// ── Zod schema ────────────────────────────────────────────────────────────────
const ruleSchema = z.object({
  code: z.string().min(1, '规则编号不能为空'),
  name: z.string().min(1, '规则名称不能为空'),
  logicOperator: z.enum(['AND', 'OR']),
  conditions: z
    .array(
      z.object({
        id: z.string(),
        field: z.string(),
        operator: z.enum(['contains', 'equals', 'not_equals', 'greater_than', 'in_list']),
        value: z.string().min(1, '匹配值不能为空'),
      })
    )
    .min(1),
  vatTransfer: z.boolean().optional(),
  vatReason: z.string().optional(),
  stampCategory: z.string().optional(),
  stampRate: z.number().min(0).max(1).optional(),
})


// ── Constants ─────────────────────────────────────────────────────────────────
const FIELD_OPTIONS = [
  { value: 'summary', label: '发票科目/摘要' },
  { value: 'ap_type', label: 'AP 凭证类型' },
  { value: 'vendor', label: '供应商名称' },
  { value: 'tax_code', label: '税收分类编码' },
  { value: 'amount', label: '不含税金额' },
  { value: 'ic_partner', label: 'IC 关联方标识' },
]

const OPERATOR_OPTIONS = [
  { value: 'contains', label: '包含' },
  { value: 'equals', label: '等于' },
  { value: 'not_equals', label: '不等于' },
  { value: 'greater_than', label: '大于' },
  { value: 'in_list', label: '在列表中 (逗号隔开)' },
]

// ── Mock fetch (TanStack Query) ───────────────────────────────────────────────
const fetchRulesMeta = async () => {
  await new Promise((r) => setTimeout(r, 300))
  return { lastSync: '2026-08-03 09:00', engine: 'v2.4.1' }
}

// ── Column helper ─────────────────────────────────────────────────────────────
const colHelper = createColumnHelper<TaxRule>()

// ── RuleEditor Modal (TanStack Form + Zod) ────────────────────────────────────
function RuleEditorModal() {
  const { isEditorOpen, editingRule } = useRuleStore()
  if (!isEditorOpen || !editingRule) return null
  return <RuleEditorForm editingRule={editingRule} />
}

function RuleEditorForm({ editingRule }: { editingRule: TaxRule }) {
  const { closeEditor, saveRule, rules } = useRuleStore()
  const isNew = !rules.some((r) => r.id === editingRule.id)

  const form = useForm({
    defaultValues: {
      code: editingRule.code,
      name: editingRule.name,
      logicOperator: editingRule.logicOperator,
      conditions: editingRule.conditions,
      vatTransfer: editingRule.action.vatTransfer ?? true,
      vatReason: editingRule.action.vatReason ?? '',
      stampCategory: editingRule.action.stampCategory ?? '',
      stampRate: editingRule.action.stampRate ?? 0.0003,
    },
    validators: {
      onSubmit: ({ value }) => {
        const result = ruleSchema.safeParse(value)
        if (!result.success) {
          return result.error.issues[0]?.message ?? '验证失败'
        }
        return undefined
      },
    },
    onSubmit: ({ value }) => {
      saveRule({
        ...editingRule,
        code: value.code,
        name: value.name,
        logicOperator: value.logicOperator,
        conditions: value.conditions,
        action:
          editingRule.category === 'vat'
            ? { vatTransfer: value.vatTransfer, vatReason: value.vatReason }
            : {
                stampDutyApplies: true,
                stampRate: value.stampRate,
                stampCategory: value.stampCategory,
              },
      })
    },
  })

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-900/40 backdrop-blur-md">
      <div className="w-full max-w-2xl bg-white dark:bg-slate-900 ring-1 ring-slate-200 dark:ring-slate-700 rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[88vh]">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3.5 border-b border-slate-200/60 dark:border-slate-800 shrink-0">
          <div className="flex items-center gap-2">
            <Sparkles className="w-4 h-4 text-sky-500" />
            <h3 className="font-bold text-[14px] text-slate-900 dark:text-slate-100">
              {isNew ? '新建计算规则' : '编辑计算规则'}
            </h3>
          </div>
          <button
            onClick={closeEditor}
            className="p-1.5 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg text-slate-400 hover:text-slate-600 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Form Body */}
        <form
          onSubmit={(e) => { e.preventDefault(); e.stopPropagation(); form.handleSubmit() }}
          className="p-5 overflow-y-auto space-y-4 flex-1 hide-scrollbar"
        >
          {/* 基本信息 */}
          <div className="grid grid-cols-2 gap-3">
            <form.Field name="code">
              {(field) => (
                <div className="space-y-1">
                  <label className="text-[11px] font-semibold text-slate-500 dark:text-slate-400">规则编号</label>
                  <input
                    value={field.state.value}
                    onChange={(e) => field.handleChange(e.target.value)}
                    onBlur={field.handleBlur}
                    className="w-full px-3 py-1.5 text-[13px] bg-white/70 dark:bg-slate-800/70 ring-1 ring-slate-200 dark:ring-slate-700 rounded-xl focus:ring-2 focus:ring-sky-500/60 outline-none text-slate-800 dark:text-slate-100 transition-shadow"
                    required
                  />
                  {field.state.meta.errors[0] && (
                    <p className="text-[11px] text-rose-500">{field.state.meta.errors[0]}</p>
                  )}
                </div>
              )}
            </form.Field>

            <form.Field name="name">
              {(field) => (
                <div className="space-y-1">
                  <label className="text-[11px] font-semibold text-slate-500 dark:text-slate-400">规则名称</label>
                  <input
                    value={field.state.value}
                    onChange={(e) => field.handleChange(e.target.value)}
                    onBlur={field.handleBlur}
                    placeholder="如：福利费发票转出"
                    className="w-full px-3 py-1.5 text-[13px] bg-white/70 dark:bg-slate-800/70 ring-1 ring-slate-200 dark:ring-slate-700 rounded-xl focus:ring-2 focus:ring-sky-500/60 outline-none text-slate-800 dark:text-slate-100 transition-shadow"
                    required
                  />
                  {field.state.meta.errors[0] && (
                    <p className="text-[11px] text-rose-500">{field.state.meta.errors[0]}</p>
                  )}
                </div>
              )}
            </form.Field>
          </div>

          {/* IF 条件组 */}
          <form.Field name="conditions">
            {(condField) => (
              <div className="space-y-2 p-3 bg-slate-50/70 dark:bg-slate-800/40 rounded-xl ring-1 ring-slate-200/60 dark:ring-slate-700/60">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="font-mono font-bold text-[12px] text-sky-600 dark:text-sky-400">IF 条件判定组</span>
                    <span className="text-slate-400 text-[11px]">多条件关系:</span>
                    <form.Field name="logicOperator">
                      {(opField) => (
                        <Select
                          items={{ OR: '任意一条 (OR)', AND: '同时满足 (AND)' }}
                          value={opField.state.value}
                          onValueChange={(v) => opField.handleChange(v as 'AND' | 'OR')}
                        >
                          <SelectTrigger className="px-2 py-0.5 h-auto text-[12px] font-bold bg-amber-500/10 text-amber-600 dark:text-amber-400 border-0 rounded-lg ring-1 ring-amber-500/20">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectItem value="OR">任意一条 (OR)</SelectItem>
                            <SelectItem value="AND">同时满足 (AND)</SelectItem>
                          </SelectContent>
                        </Select>
                      )}
                    </form.Field>
                  </div>
                  <button
                    type="button"
                    onClick={() =>
                      condField.pushValue({
                        id: `c-${Date.now()}`,
                        field: 'summary',
                        operator: 'contains',
                        value: '',
                      })
                    }
                    className="flex items-center gap-1 px-2.5 py-1 text-[12px] bg-sky-500/10 hover:bg-sky-500/20 text-sky-600 dark:text-sky-300 rounded-lg font-medium transition-colors"
                  >
                    <Plus className="w-3 h-3" /> 增加子条件
                  </button>
                </div>

                <div className="space-y-2 pt-1">
                  {condField.state.value.map((cond, index) => (
                    <div key={cond.id} className="flex items-center gap-2 p-2 bg-white/80 dark:bg-slate-800/80 rounded-xl ring-1 ring-slate-200/80 dark:ring-slate-700">
                      <span className="text-slate-400 font-mono text-[11px] shrink-0">#{index + 1}</span>
                      <Select
                        items={FIELD_OPTIONS}
                        value={cond.field}
                        onValueChange={(v) => {
                          const next = [...condField.state.value]
                          next[index] = { ...next[index], field: v as string }
                          condField.handleChange(next)
                        }}
                      >
                        <SelectTrigger className="px-2 py-1 h-auto text-[12px] bg-slate-100 dark:bg-slate-700 border-0 rounded-lg text-slate-800 dark:text-slate-100">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {FIELD_OPTIONS.map((f) => <SelectItem key={f.value} value={f.value}>{f.label}</SelectItem>)}
                        </SelectContent>
                      </Select>
                      <Select
                        items={OPERATOR_OPTIONS}
                        value={cond.operator}
                        onValueChange={(v) => {
                          const next = [...condField.state.value]
                          next[index] = { ...next[index], operator: v as RuleCondition['operator'] }
                          condField.handleChange(next)
                        }}
                      >
                        <SelectTrigger className="px-2 py-1 h-auto text-[12px] bg-slate-100 dark:bg-slate-700 border-0 rounded-lg text-slate-800 dark:text-slate-100">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {OPERATOR_OPTIONS.map((o) => <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>)}
                        </SelectContent>
                      </Select>
                      <input
                        type="text"
                        value={cond.value}
                        onChange={(e) => {
                          const next = [...condField.state.value]
                          next[index] = { ...next[index], value: e.target.value }
                          condField.handleChange(next)
                        }}
                        placeholder="输入匹配关键词..."
                        className="flex-1 px-2.5 py-1 text-[13px] bg-slate-50 dark:bg-slate-900 ring-1 ring-slate-200 dark:ring-slate-700 rounded-lg outline-none text-slate-800 dark:text-slate-100 focus:ring-sky-500/50"
                      />
                      {condField.state.value.length > 1 && (
                        <button
                          type="button"
                          onClick={() => condField.removeValue(index)}
                          className="p-1 hover:bg-red-500/10 rounded text-slate-400 hover:text-red-500 transition-colors"
                        >
                          <X className="w-3.5 h-3.5" />
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </form.Field>

          {/* THEN 动作 */}
          <div className="space-y-2 p-3 bg-emerald-50/50 dark:bg-emerald-950/20 rounded-xl ring-1 ring-emerald-500/20">
            <span className="font-mono font-bold text-[12px] text-emerald-600 dark:text-emerald-400">THEN 执行动作</span>
            {editingRule.category === 'vat' ? (
              <div className="grid grid-cols-2 gap-3 pt-1">
                <form.Field name="vatTransfer">
                  {(f) => (
                    <div className="space-y-1">
                      <label className="text-[11px] font-semibold text-slate-500 dark:text-slate-400">进项标识判定</label>
                      <Select
                        items={{ true: '标识为：应该转出', false: '标识为：不转出' }}
                        value={f.state.value ? 'true' : 'false'}
                        onValueChange={(v) => f.handleChange(v === 'true')}
                      >
                        <SelectTrigger className="w-full px-3 py-1.5 h-auto text-[13px] bg-white dark:bg-slate-800 ring-1 ring-slate-200 dark:ring-slate-700 rounded-xl text-slate-800 dark:text-slate-100">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="true">标识为：应该转出</SelectItem>
                          <SelectItem value="false">标识为：不转出</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>
                  )}
                </form.Field>
                <form.Field name="vatReason">
                  {(f) => (
                    <div className="space-y-1">
                      <label className="text-[11px] font-semibold text-slate-500 dark:text-slate-400">转出原因说明</label>
                      <input
                        value={f.state.value ?? ''}
                        onChange={(e) => f.handleChange(e.target.value)}
                        placeholder="如：集体福利不可抵扣"
                        className="w-full px-3 py-1.5 text-[13px] bg-white dark:bg-slate-800 ring-1 ring-slate-200 dark:ring-slate-700 rounded-xl outline-none text-slate-800 dark:text-slate-100 focus:ring-2 focus:ring-sky-500/50"
                      />
                    </div>
                  )}
                </form.Field>
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-3 pt-1">
                <form.Field name="stampCategory">
                  {(f) => (
                    <div className="space-y-1">
                      <label className="text-[11px] font-semibold text-slate-500 dark:text-slate-400">印花税合同分类</label>
                      <input
                        value={f.state.value ?? ''}
                        onChange={(e) => f.handleChange(e.target.value)}
                        placeholder="如：买卖合同"
                        className="w-full px-3 py-1.5 text-[13px] bg-white dark:bg-slate-800 ring-1 ring-slate-200 dark:ring-slate-700 rounded-xl outline-none text-slate-800 dark:text-slate-100 focus:ring-2 focus:ring-sky-500/50"
                      />
                    </div>
                  )}
                </form.Field>
                <form.Field name="stampRate">
                  {(f) => (
                    <div className="space-y-1">
                      <label className="text-[11px] font-semibold text-slate-500 dark:text-slate-400">印花税税率 (如 0.0003)</label>
                      <input
                        type="number"
                        step="0.00001"
                        value={f.state.value ?? 0}
                        onChange={(e) => f.handleChange(parseFloat(e.target.value) || 0)}
                        className="w-full px-3 py-1.5 text-[13px] bg-white dark:bg-slate-800 ring-1 ring-slate-200 dark:ring-slate-700 rounded-xl outline-none text-slate-800 dark:text-slate-100 focus:ring-2 focus:ring-sky-500/50"
                      />
                    </div>
                  )}
                </form.Field>
              </div>
            )}
          </div>

          {/* 底部操作 */}
          <form.Subscribe selector={(s) => s.errors}>
            {(errors) =>
              errors.length > 0 ? (
                <p className="text-[12px] text-rose-500 px-1">{errors.join(', ')}</p>
              ) : null
            }
          </form.Subscribe>

          <div className="pt-1 flex items-center justify-end gap-2 border-t border-slate-200/60 dark:border-slate-800">
            <button
              type="button"
              onClick={closeEditor}
              className="px-4 py-1.5 rounded-xl bg-slate-200/60 dark:bg-slate-800 text-slate-700 dark:text-slate-300 text-[13px] font-medium hover:bg-slate-200 transition-colors"
            >
              取消
            </button>
            <form.Subscribe selector={(s) => s.isSubmitting}>
              {(isSubmitting) => (
                <button
                  type="submit"
                  disabled={isSubmitting}
                  className="flex items-center gap-1.5 px-5 py-1.5 rounded-xl bg-[#0f172a] hover:bg-[#1e293b] dark:bg-sky-500 dark:hover:bg-sky-400 text-white text-[13px] font-semibold shadow-md disabled:opacity-60 transition-colors"
                >
                  <Check className="w-3.5 h-3.5" />
                  保存并应用规则
                </button>
              )}
            </form.Subscribe>
          </div>
        </form>
      </div>
    </div>
  )
}

// ── Main RuleEngine component ──────────────────────────────────────────────────
export default function RuleEngine() {
  const {
    rules,
    activeCategory,
    searchQuery,
    statusFilter,
    setActiveCategory,
    setSearchQuery,
    setStatusFilter,
    toggleEnabled,
    deleteRule,
    movePriority,
    openEditor,
  } = useRuleStore()

  // TanStack Query: 拉取引擎元数据
  const { data: meta } = useQuery({
    queryKey: ['rules-meta'],
    queryFn: fetchRulesMeta,
  })

  // 过滤 + 排序（纯计算，不需要额外 state）
  const filteredRules = useMemo(
    () =>
      rules
        .filter((r) => r.category === activeCategory)
        .filter((r) => {
          if (statusFilter === 'enabled') return r.enabled
          if (statusFilter === 'disabled') return !r.enabled
          return true
        })
        .filter(
          (r) =>
            r.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
            r.code.toLowerCase().includes(searchQuery.toLowerCase())
        )
        .sort((a, b) => a.priority - b.priority),
    [rules, activeCategory, statusFilter, searchQuery]
  )

  // TanStack Table 列定义
  const columns = useMemo(
    () => [
      colHelper.accessor('priority', {
        header: '#',
        cell: (info) => (
          <div className="flex flex-col items-center justify-center w-7 h-7 rounded-lg bg-slate-200/50 dark:bg-slate-700/50 text-slate-700 dark:text-slate-300 font-bold text-[12px] ring-1 ring-black/5 mx-auto">
            #{info.getValue()}
          </div>
        ),
        meta: { className: 'px-3 py-2 w-12 text-center' },
      }),
      colHelper.display({
        id: 'rule_body',
        header: '规则',
        cell: ({ row }) => {
          const rule = row.original
          return (
            <div className="space-y-1">
              <div className="flex items-center gap-2">
                <span className="font-mono text-[11px] font-semibold text-slate-400 dark:text-slate-500">
                  {rule.code}
                </span>
                <span className="font-semibold text-[13px] text-slate-800 dark:text-slate-100">
                  {rule.name}
                </span>
                <span className="px-1.5 rounded text-[10px] font-medium bg-slate-200/60 dark:bg-slate-700/60 text-slate-500 dark:text-slate-300">
                  LE: {rule.leAppliesTo.join(', ')}
                </span>
              </div>
              <div className="flex items-center gap-1.5 flex-wrap text-[12px]">
                <span className="font-mono font-bold text-sky-600 dark:text-sky-400">IF</span>
                {rule.conditions.map((c, cIdx) => (
                  <React.Fragment key={c.id}>
                    {cIdx > 0 && (
                      <span className="px-1 rounded text-[10px] font-bold bg-amber-500/10 text-amber-600 dark:text-amber-400">
                        {rule.logicOperator}
                      </span>
                    )}
                    <span className="px-2 py-0.5 rounded-md bg-white/80 dark:bg-slate-900/80 ring-1 ring-slate-200 dark:ring-slate-700 text-slate-700 dark:text-slate-300">
                      {FIELD_OPTIONS.find((f) => f.value === c.field)?.label}
                      <span className="text-sky-500 font-semibold mx-1">
                        {OPERATOR_OPTIONS.find((o) => o.value === c.operator)?.label}
                      </span>
                      <span className="font-semibold text-slate-900 dark:text-white">"{c.value}"</span>
                    </span>
                  </React.Fragment>
                ))}
                <span className="font-mono font-bold text-emerald-600 dark:text-emerald-400 ml-1">THEN</span>
                {rule.category === 'vat' ? (
                  <span className="px-2 py-0.5 rounded-md bg-emerald-500/10 text-emerald-700 dark:text-emerald-300 font-medium ring-1 ring-emerald-500/20">
                    {rule.action.vatTransfer ? '转出' : '不转出'}
                    {rule.action.vatReason && ` (${rule.action.vatReason})`}
                  </span>
                ) : (
                  <span className="px-2 py-0.5 rounded-md bg-blue-500/10 text-blue-700 dark:text-blue-300 font-medium ring-1 ring-blue-500/20">
                    计提【{rule.action.stampCategory}】税率 {((rule.action.stampRate || 0) * 100).toFixed(3)}%
                  </span>
                )}
              </div>
            </div>
          )
        },
        meta: { className: 'px-3 py-2.5' },
      }),
      colHelper.display({
        id: 'actions',
        header: '操作',
        cell: ({ row, table }) => {
          const rule = row.original
          const idx = row.index
          const total = table.getRowModel().rows.length
          return (
            <div className="flex items-center gap-2 justify-end">
              {/* 优先级 */}
              <div className="flex items-center bg-slate-200/40 dark:bg-slate-700/40 rounded-lg p-0.5">
                <button
                  onClick={() => movePriority(rule.id, 'up')}
                  disabled={idx === 0}
                  className="p-1 hover:bg-white dark:hover:bg-slate-600 rounded disabled:opacity-30 text-slate-600 dark:text-slate-300 transition-colors"
                  title="提高优先级"
                >
                  <ArrowUp className="w-3 h-3" />
                </button>
                <button
                  onClick={() => movePriority(rule.id, 'down')}
                  disabled={idx === total - 1}
                  className="p-1 hover:bg-white dark:hover:bg-slate-600 rounded disabled:opacity-30 text-slate-600 dark:text-slate-300 transition-colors"
                  title="降低优先级"
                >
                  <ArrowDown className="w-3 h-3" />
                </button>
              </div>
              {/* 启用/禁用 */}
              <button
                onClick={() => toggleEnabled(rule.id)}
                className={`flex items-center gap-1 px-2.5 py-1 rounded-lg text-[12px] font-medium transition-all ${
                  rule.enabled
                    ? 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300 ring-1 ring-emerald-500/30'
                    : 'bg-slate-200 dark:bg-slate-700 text-slate-500'
                }`}
              >
                {rule.enabled ? (
                  <><CheckCircle2 className="w-3 h-3 text-emerald-600" /> 已启用</>
                ) : (
                  <><XCircle className="w-3 h-3 text-slate-400" /> 已禁用</>
                )}
              </button>
              {/* 编辑 */}
              <button
                onClick={() => openEditor(rule)}
                className="p-1.5 hover:bg-white/80 dark:hover:bg-slate-700 rounded-lg text-slate-500 dark:text-slate-300 transition-colors"
                title="编辑规则"
              >
                <Edit3 className="w-3.5 h-3.5" />
              </button>
              {/* 删除 */}
              <button
                onClick={() => {
                  if (confirm('确定要删除这条计算规则吗？此操作无法撤销。')) deleteRule(rule.id)
                }}
                className="p-1.5 hover:bg-red-500/10 rounded-lg text-slate-400 hover:text-red-500 transition-colors"
                title="删除规则"
              >
                <Trash2 className="w-3.5 h-3.5" />
              </button>
            </div>
          )
        },
        meta: { className: 'px-4 py-2.5 text-right whitespace-nowrap' },
      }),
    ],
    [movePriority, toggleEnabled, openEditor, deleteRule]
  )

  const table = useReactTable({
    data: filteredRules,
    columns,
    getCoreRowModel: getCoreRowModel(),
  })

  return (
    <div className="w-full h-full flex flex-col space-y-3 select-none">
      {/* 标题区 */}
      <div className="flex items-center justify-between shrink-0">
        <div className="space-y-0.5">
          <div className="flex items-center gap-2">
            <SlidersHorizontal className="w-4 h-4 text-sky-600 dark:text-sky-400" />
            <h2 className="text-[15px] font-bold text-slate-800 dark:text-slate-100 tracking-tight">
              规则配置引擎
            </h2>
            <span className="px-2 py-0.5 rounded-full text-[10px] font-medium bg-sky-500/10 text-sky-600 dark:text-sky-300 ring-1 ring-sky-500/20">
              SQLite 驱动
            </span>
            {meta && (
              <span className="text-[11px] text-slate-400 dark:text-slate-500 ml-1">
                引擎 {meta.engine} · 同步于 {meta.lastSync}
              </span>
            )}
          </div>
          <p className="text-[12px] text-slate-500 dark:text-slate-400">
            通过灵活的匹配规则，自动化处理发票明细的进项转出及印花税计提判定。
          </p>
        </div>

        {/* 类别切换 */}
        <div className="flex p-1 bg-slate-100 dark:bg-slate-900/60 ring-1 ring-slate-200 dark:ring-white/10 rounded-xl shadow-sm shrink-0">
          <button
            onClick={() => setActiveCategory('vat')}
            className={`flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-[13px] font-semibold transition-all ${
              activeCategory === 'vat'
                ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-white shadow-md ring-1 ring-black/5'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
            }`}
          >
            <ShieldAlert className="w-3.5 h-3.5 text-amber-500" />
            进项转出规则
          </button>
          <button
            onClick={() => setActiveCategory('stamp')}
            className={`flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-[13px] font-semibold transition-all ${
              activeCategory === 'stamp'
                ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-white shadow-md ring-1 ring-black/5'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
            }`}
          >
            <Tag className="w-3.5 h-3.5 text-blue-500" />
            印花税计算规则
          </button>
        </div>
      </div>

      {/* 控制栏 */}
      <LiquidCard noPadding className="px-3 py-2.5 shrink-0">
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2.5 flex-1 max-w-md">
            <div className="flex-1">
              <LiquidInput
                icon={Search}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="搜索规则名称或编号..."
                clearable
                onClear={() => setSearchQuery('')}
              />
            </div>
            <div className="w-36 shrink-0">
              <LiquidSelect value={statusFilter} onChange={(e) => setStatusFilter(e.target.value as any)}>
                <option value="all">状态: 全部</option>
                <option value="enabled">已启用</option>
                <option value="disabled">已禁用</option>
              </LiquidSelect>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <label className="cursor-pointer">
              <LiquidButton variant="secondary" icon={FileSpreadsheet} type="button" onClick={() => document.getElementById('excel-upload')?.click()}>
                Excel 导入
              </LiquidButton>
              <input
                id="excel-upload"
                type="file"
                className="hidden"
                accept=".xlsx,.xls"
                onChange={(e) => {
                  if (e.target.files?.[0]) {
                    alert(`已选择: ${e.target.files[0].name}\n(将对接 Tauri Rust 后端解析并批量生成规则)`)
                    e.target.value = ''
                  }
                }}
              />
            </label>
            <LiquidButton variant="primary" icon={Plus} onClick={() => openEditor()}>
              新建{activeCategory === 'vat' ? '转出' : '印花税'}规则
            </LiquidButton>
          </div>
        </div>
      </LiquidCard>

      {/* 规则表格 */}
      <LiquidCard noPadding className="flex-1 flex flex-col min-h-0">
        {filteredRules.length === 0 ? (
          <div className="flex-1 flex flex-col items-center justify-center py-16 text-slate-400 gap-2">
            <Layers className="w-10 h-10 opacity-40" />
            <p className="text-[13px]">未找到符合条件的计算规则</p>
          </div>
        ) : (
          <div className="flex-1 overflow-auto hide-scrollbar">
            <table className="w-full border-collapse">
              <thead className="sticky top-0 z-10 bg-slate-50 dark:bg-slate-800">
                {table.getHeaderGroups().map((hg) => (
                  <tr key={hg.id} className="text-[12px] font-semibold text-slate-500 dark:text-slate-400 border-b border-slate-200/70 dark:border-slate-700/80">
                    {hg.headers.map((h) => (
                      <th key={h.id} className={(h.column.columnDef.meta as any)?.className ?? 'px-3 py-2'}>
                        {flexRender(h.column.columnDef.header, h.getContext())}
                      </th>
                    ))}
                  </tr>
                ))}
              </thead>
              <tbody className="divide-y divide-slate-100/60 dark:divide-slate-800/40">
                {table.getRowModel().rows.map((row) => {
                  const rule = row.original
                  return (
                    <tr
                      key={row.id}
                      className={`transition-colors ${
                        rule.enabled
                          ? 'hover:bg-sky-500/[0.04] dark:hover:bg-sky-400/[0.06]'
                          : 'opacity-50 bg-slate-50/30 dark:bg-slate-900/10'
                      }`}
                    >
                      {row.getVisibleCells().map((cell) => (
                        <td key={cell.id} className={(cell.column.columnDef.meta as any)?.className ?? 'px-3 py-2'}>
                          {flexRender(cell.column.columnDef.cell, cell.getContext())}
                        </td>
                      ))}
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
        )}
      </LiquidCard>

      {/* 编辑器 Modal */}
      <RuleEditorModal />
    </div>
  )
}
