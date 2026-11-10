import React, { useState } from "react"
import { motion } from "motion/react"
import {
  TrendUp as TrendingUp,
  Users,
  CreditCard,
  ArrowUpRight,
  Bell,
  Gear as Settings,
  MagnifyingGlass as Search,
  Export,
  ArrowCounterClockwise,
} from "@phosphor-icons/react"
import { createColumnHelper, flexRender, getCoreRowModel, useReactTable } from "@tanstack/react-table-v8"
import { useQuery } from "@tanstack/react-query"
import { useForm } from "@tanstack/react-form"
import { z } from "zod"
import { format, parse } from "date-fns"
import { Win11DatePicker } from "@/components/ui/win11-date-picker"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"

type Invoice = {
  id: string
  date: string
  company: string
  amount: string
  type: string
  status: string
}

const invoiceData: Invoice[] = [
  { id: "INV-2026-001", date: "2026-07-30 14:23:05", company: "科技实业有限公司",    amount: "¥ 45,000.00",      type: "服务费结转", status: "已匹配" },
  { id: "INV-2026-002", date: "2026-07-30 10:15:22", company: "云端网络科技",        amount: "¥ 12,800.00",      type: "软件授权费", status: "解析中" },
  { id: "INV-2026-003", date: "2026-07-29 16:45:10", company: "鼎盛供应链管理",      amount: "¥ 89,200.00",      type: "硬件采购款", status: "异常" },
  { id: "INV-2026-004", date: "2026-07-29 09:30:00", company: "寰宇物流集团",        amount: "¥ 214,000.00",     type: "服务费结转", status: "已匹配" },
  { id: "INV-2026-005", date: "2026-07-28 11:20:45", company: "蓝海创投基金",        amount: "¥ 1,500,000.00",   type: "股权投资",   status: "已匹配" },
  { id: "INV-2026-006", date: "2026-07-28 08:15:30", company: "光速传媒广告",        amount: "¥ 35,600.00",      type: "广告推广费", status: "待审核" },
]

const fetchInvoices = async (): Promise<Invoice[]> => {
  await new Promise(resolve => setTimeout(resolve, 800))
  return invoiceData
}

const columnHelper = createColumnHelper<Invoice>()

const STATUS_MAP: Record<string, { cls: string; dot: string }> = {
  "已匹配": { cls: "badge-matched", dot: "var(--accent-green)" },
  "异常":   { cls: "badge-error",   dot: "var(--accent-red)" },
  "解析中": { cls: "badge-parsing", dot: "var(--blue-light)" },
  "待审核": { cls: "badge-pending", dot: "var(--accent-amber)" },
}

const columns = [
  columnHelper.display({
    id: "select",
    header: () => (
      <div
        className="w-3.5 h-3.5 rounded-[3px] mx-auto"
        style={{ border: '1px solid var(--glass-border)', background: 'var(--input-bg)' }}
      />
    ),
    cell: () => (
      <div
        className="w-3.5 h-3.5 rounded-[3px] mx-auto"
        style={{ border: '1px solid var(--glass-border)', background: 'var(--input-bg)' }}
      />
    ),
    meta: { className: "px-4 py-2.5 w-10 text-center" }
  }),
  columnHelper.accessor("id", {
    header: "流水号",
    cell: info => (
      <span className="font-medium font-mono text-[12px]" style={{ color: 'var(--blue-primary)' }}>
        {info.getValue()}
      </span>
    ),
    meta: { className: "px-3 py-2.5" }
  }),
  columnHelper.accessor("date", {
    header: "发生时间",
    cell: info => <span style={{ color: 'var(--text-muted)' }}>{info.getValue()}</span>,
    meta: { className: "px-3 py-2.5" }
  }),
  columnHelper.accessor("company", {
    header: "交易对手名称",
    cell: info => <span className="font-semibold" style={{ color: 'var(--text-primary)' }}>{info.getValue()}</span>,
    meta: { className: "px-3 py-2.5" }
  }),
  columnHelper.accessor("amount", {
    header: () => <div className="text-right">交易金额</div>,
    cell: info => (
      <div className="font-bold text-right tabular-nums" style={{ color: 'var(--text-primary)' }}>
        {info.getValue()}
      </div>
    ),
    meta: { className: "px-3 py-2.5 text-right" }
  }),
  columnHelper.accessor("type", {
    header: "业务类型",
    cell: info => <span style={{ color: 'var(--text-secondary)' }}>{info.getValue()}</span>,
    meta: { className: "px-3 py-2.5" }
  }),
  columnHelper.accessor("status", {
    header: "状态",
    cell: info => {
      const status = info.getValue()
      const s = STATUS_MAP[status] ?? STATUS_MAP["待审核"]
      return (
        <span className={`inline-flex items-center gap-1.5 text-[11px] px-2.5 py-0.5 rounded-full font-semibold ${s.cls}`}>
          <span className="w-1 h-1 rounded-full" style={{ background: s.dot, boxShadow: `0 0 4px ${s.dot}` }} />
          {status}
        </span>
      )
    },
    meta: { className: "px-3 py-2.5" }
  }),
  columnHelper.display({
    id: "actions",
    header: () => <div className="text-right">操作</div>,
    cell: () => (
      <div className="text-right space-x-3">
        <button
          className="text-[12px] font-medium transition-colors"
          style={{ color: 'var(--blue-primary)' }}
          onMouseEnter={(e) => (e.currentTarget.style.color = 'var(--blue-light)')}
          onMouseLeave={(e) => (e.currentTarget.style.color = 'var(--blue-primary)')}
        >
          查看
        </button>
        <button
          className="text-[12px] font-medium transition-colors"
          style={{ color: 'var(--text-muted)' }}
          onMouseEnter={(e) => (e.currentTarget.style.color = 'var(--text-secondary)')}
          onMouseLeave={(e) => (e.currentTarget.style.color = 'var(--text-muted)')}
        >
          编辑
        </button>
      </div>
    ),
    meta: { className: "px-4 py-2.5 text-right" }
  }),
]

// 玻璃卡片样式
const glassCard: React.CSSProperties = {
  background: 'var(--card-bg)',
  backdropFilter: 'var(--glass-blur)',
  WebkitBackdropFilter: 'var(--glass-blur)',
  border: '1px solid var(--card-border)',
  boxShadow: 'var(--card-shadow)',
  borderRadius: '14px',
  position: 'relative',
  overflow: 'hidden',
}

const glassInput: React.CSSProperties = {
  background: 'var(--input-bg)',
  backdropFilter: 'blur(8px)',
  WebkitBackdropFilter: 'blur(8px)',
  border: '1px solid var(--input-border)',
  color: 'var(--input-text)',
  borderRadius: '8px',
  fontSize: '13px',
  padding: '6px 12px',
  outline: 'none',
  width: '100%',
  transition: 'border-color 0.15s ease, box-shadow 0.15s ease',
}

const focusHandlers = {
  onFocus: (e: React.FocusEvent<HTMLElement>) => {
    e.currentTarget.style.borderColor = 'var(--input-focus-ring)'
    e.currentTarget.style.boxShadow = '0 0 0 3px color-mix(in oklch, var(--input-focus-ring) 18%, transparent)'
  },
  onBlur: (e: React.FocusEvent<HTMLElement>) => {
    e.currentTarget.style.borderColor = 'var(--input-border)'
    e.currentTarget.style.boxShadow = 'none'
  },
}

// 指标卡配置
const STAT_CARDS = [
  {
    label: '本月已核销金额',
    value: '¥ 1,284,500',
    sub: '较上月增长 ¥142,000',
    badge: '+12.5%',
    badgeColor: 'var(--accent-green)',
    badgeBg: 'var(--accent-green-bg)',
    Icon: CreditCard,
    iconGradient: 'linear-gradient(135deg, #3b6ef0 0%, #6366f1 100%)',
    iconGlow: 'rgba(59,110,240,0.35)',
  },
  {
    label: '待确认付款条目',
    value: '42 笔',
    sub: '预计 2 个工作日内处理完成',
    badge: '需人工复核',
    badgeColor: 'var(--accent-amber)',
    badgeBg: 'var(--accent-amber-bg)',
    Icon: TrendingUp,
    iconGradient: 'linear-gradient(135deg, #f59e0b 0%, #f97316 100%)',
    iconGlow: 'rgba(245,158,11,0.35)',
  },
  {
    label: '活跃企业客户端',
    value: '128 家',
    sub: '本月新增 5 家对接企业',
    badge: '+4.2%',
    badgeColor: 'var(--accent-violet)',
    badgeBg: 'var(--accent-violet-bg)',
    Icon: Users,
    iconGradient: 'linear-gradient(135deg, #7c3aed 0%, #a855f7 100%)',
    iconGlow: 'rgba(124,58,237,0.35)',
  },
]

export default function DashboardContent() {
  const [activeTab, setActiveTab] = useState("overview")

  const tabs = [
    { id: "overview", label: "数据概览" },
    { id: "invoices", label: "发票匹配" },
    { id: "logs",     label: "流水日志" },
  ]

  const { data: remoteData, isLoading } = useQuery({
    queryKey: ['dashboard-invoices'],
    queryFn: fetchInvoices,
    staleTime: 1000 * 60 * 5,
  })

  const table = useReactTable({
    data: remoteData ?? [],
    columns,
    getCoreRowModel: getCoreRowModel(),
  })

  const searchSchema = z.object({
    keyword: z.string(),
    type: z.string(),
    startDate: z.string(),
    endDate: z.string(),
  })

  const form = useForm({
    defaultValues: { keyword: '', type: '全部类型', startDate: '', endDate: '' },
    validators: {
      onSubmit: ({ value }) => {
        const result = searchSchema.safeParse(value)
        if (!result.success) return result.error.issues[0]?.message
        return undefined
      },
    },
    onSubmit: async ({ value }) => { console.log('查询:', value) },
  })

  return (
    <div className="flex flex-col flex-1 min-h-0 w-full overflow-y-auto pb-4 thin-scrollbar gap-4">

      {/* 1. 页头 */}
      <header className="flex items-center justify-between shrink-0 pt-1">
        <div>
          <h1 className="text-[18px] font-bold tracking-tight" style={{ color: 'var(--text-primary)' }}>
            财务结算控制台
          </h1>
          <p className="text-[13px] mt-0.5" style={{ color: 'var(--text-muted)' }}>
            实时监控月度开票与核销数据
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[13px] font-medium transition-all"
            style={{
              background: 'var(--glass-bg)',
              backdropFilter: 'blur(12px)',
              WebkitBackdropFilter: 'blur(12px)',
              border: '1px solid var(--glass-border)',
              color: 'var(--text-secondary)',
            }}
            onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--glass-bg-strong)')}
            onMouseLeave={(e) => (e.currentTarget.style.background = 'var(--glass-bg)')}
          >
            <Bell weight="duotone" className="w-3.5 h-3.5" />
            系统通知
          </button>
          <button
            className="flex items-center gap-1.5 px-4 py-1.5 rounded-xl text-[13px] font-semibold text-white transition-all"
            style={{
              background: 'linear-gradient(135deg, var(--blue-primary) 0%, color-mix(in oklch, var(--blue-primary) 70%, #7c3aed) 100%)',
              boxShadow: '0 4px 14px color-mix(in oklch, var(--blue-primary) 35%, transparent)',
            }}
            onMouseEnter={(e) => (e.currentTarget.style.boxShadow = '0 6px 20px color-mix(in oklch, var(--blue-primary) 50%, transparent)')}
            onMouseLeave={(e) => (e.currentTarget.style.boxShadow = '0 4px 14px color-mix(in oklch, var(--blue-primary) 35%, transparent)')}
          >
            <Export weight="bold" className="w-3.5 h-3.5" />
            导出报表
          </button>
        </div>
      </header>

      {/* 2. Tab 切换 */}
      <div className="flex items-center justify-between shrink-0">
        <div
          className="flex p-1 rounded-xl gap-0.5 bg-slate-100 dark:bg-slate-900/60"
        >
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className="relative px-4 py-1 text-[13px] font-medium rounded-lg transition-colors z-10"
              style={{ color: activeTab === tab.id ? 'var(--tab-active-text)' : 'var(--text-muted)' }}
            >
              {activeTab === tab.id && (
                <motion.div
                  layoutId="dash-tab-indicator"
                  className="absolute inset-0 rounded-lg -z-10"
                  style={{
                    background: 'var(--segment-active-bg)',
                    backdropFilter: 'blur(12px)',
                    WebkitBackdropFilter: 'blur(12px)',
                    border: '1px solid var(--glass-border)',
                    boxShadow: '0 2px 8px rgba(100,130,255,0.10)',
                  }}
                  transition={{ type: "spring", bounce: 0.18, duration: 0.45 }}
                />
              )}
              {tab.label}
            </button>
          ))}
        </div>
        <span
          className="text-[12px] px-3 py-1 rounded-lg"
          style={{
            color: 'var(--text-muted)',
            background: 'var(--glass-bg)',
            backdropFilter: 'blur(8px)',
            WebkitBackdropFilter: 'blur(8px)',
            border: '1px solid var(--glass-border)',
          }}
        >
          更新于: 今天 16:45
        </span>
      </div>

      {/* 3. 指标卡片 */}
      <div className="grid grid-cols-3 gap-4 shrink-0">
        {STAT_CARDS.map((card, i) => (
          <div
            key={i}
            className="p-4 flex flex-col gap-3 group transition-all duration-200"
            style={{
              ...glassCard,
              cursor: 'default',
            }}
            onMouseEnter={(e) => {
              (e.currentTarget as HTMLElement).style.boxShadow = 'var(--card-shadow-hover)'
              ;(e.currentTarget as HTMLElement).style.borderColor = 'var(--glass-border)'
            }}
            onMouseLeave={(e) => {
              (e.currentTarget as HTMLElement).style.boxShadow = 'var(--card-shadow)'
              ;(e.currentTarget as HTMLElement).style.borderColor = 'var(--card-border)'
            }}
          >
            {/* 顶部高光 */}
            <div
              className="absolute top-0 left-0 right-0 h-px"
              style={{ background: 'linear-gradient(90deg, transparent, rgba(255,255,255,0.25), transparent)' }}
            />

            <div className="flex items-center justify-between">
              <span className="text-[13px] font-medium" style={{ color: 'var(--text-muted)' }}>
                {card.label}
              </span>
              <div
                className="w-8 h-8 rounded-xl flex items-center justify-center"
                style={{
                  background: card.iconGradient,
                  boxShadow: `0 4px 12px ${card.iconGlow}`,
                }}
              >
                <card.Icon weight="bold" className="w-4 h-4 text-white" />
              </div>
            </div>

            <div className="flex items-baseline justify-between">
              <span className="text-[22px] font-bold tracking-tight tabular-nums" style={{ color: 'var(--text-primary)' }}>
                {card.value}
              </span>
              <span
                className="text-[11px] font-semibold px-2.5 py-0.5 rounded-full flex items-center gap-0.5"
                style={{ background: card.badgeBg, color: card.badgeColor }}
              >
                {card.badge.startsWith('+') && <ArrowUpRight weight="bold" className="w-3 h-3" />}
                {card.badge}
              </span>
            </div>

            <p className="text-[12px]" style={{ color: 'var(--text-faint)' }}>{card.sub}</p>
          </div>
        ))}
      </div>

      {/* 4. 查询条件面板 */}
      <div className="p-4 shrink-0" style={glassCard}>
        {/* 顶部高光 */}
        <div
          className="absolute top-0 left-0 right-0 h-px"
          style={{ background: 'linear-gradient(90deg, transparent, rgba(255,255,255,0.20), transparent)' }}
        />

        <form onSubmit={(e) => { e.preventDefault(); e.stopPropagation(); form.handleSubmit() }}>
          <div className="flex justify-between items-center mb-4">
            <h3 className="text-[14px] font-bold flex items-center gap-2" style={{ color: 'var(--text-primary)' }}>
              <div
                className="w-5 h-5 rounded-md flex items-center justify-center"
                style={{ background: 'var(--blue-pale-2)' }}
              >
                <Search className="w-3 h-3" style={{ color: 'var(--blue-primary)' }} />
              </div>
              快速数据检索
            </h3>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => form.reset()}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[13px] font-medium transition-all"
                style={{
                  background: 'var(--glass-bg)',
                  backdropFilter: 'blur(8px)',
                  WebkitBackdropFilter: 'blur(8px)',
                  border: '1px solid var(--glass-border)',
                  color: 'var(--text-secondary)',
                }}
                onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--glass-bg-strong)')}
                onMouseLeave={(e) => (e.currentTarget.style.background = 'var(--glass-bg)')}
              >
                <ArrowCounterClockwise className="w-3 h-3" />
                重置
              </button>
              <button
                type="submit"
                className="px-4 py-1.5 rounded-lg text-[13px] font-semibold text-white transition-all"
                style={{
                  background: 'linear-gradient(135deg, var(--blue-primary) 0%, color-mix(in oklch, var(--blue-primary) 70%, #7c3aed) 100%)',
                  boxShadow: '0 2px 10px color-mix(in oklch, var(--blue-primary) 30%, transparent)',
                }}
              >
                查询
              </button>
            </div>
          </div>

          <div className="grid grid-cols-5 gap-3">
            <div className="space-y-1.5">
              <label className="text-[11px] font-semibold pl-0.5 block" style={{ color: 'var(--text-muted)' }}>
                流水号 / 关键字
              </label>
              <form.Field name="keyword">
                {(field) => (
                  <input
                    value={field.state.value}
                    onChange={(e) => field.handleChange(e.target.value)}
                    type="text"
                    placeholder="请输入流水号、名称..."
                    style={{ ...glassInput }}
                    {...focusHandlers}
                  />
                )}
              </form.Field>
            </div>

            <div className="space-y-1.5">
              <label className="text-[11px] font-semibold pl-0.5 block" style={{ color: 'var(--text-muted)' }}>
                业务类型
              </label>
              <div className="relative">
                <form.Field name="type">
                  {(field) => (
                    <Select
                      items={{ 全部类型: '全部类型', 服务费结转: '服务费结转', 软件授权费: '软件授权费', 硬件采购款: '硬件采购款' }}
                      value={field.state.value}
                      onValueChange={(v) => field.handleChange(v as string)}
                    >
                      <SelectTrigger className="w-full h-auto" style={{ ...glassInput }} {...focusHandlers}>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="全部类型">全部类型</SelectItem>
                        <SelectItem value="服务费结转">服务费结转</SelectItem>
                        <SelectItem value="软件授权费">软件授权费</SelectItem>
                        <SelectItem value="硬件采购款">硬件采购款</SelectItem>
                      </SelectContent>
                    </Select>
                  )}
                </form.Field>
              </div>
            </div>

            <div className="space-y-1.5 col-span-2">
              <label className="text-[11px] font-semibold pl-0.5 block" style={{ color: 'var(--text-muted)' }}>
                交易日期区间
              </label>
              <div className="flex items-center gap-1.5">
                <div className="flex-1 min-w-0">
                  <form.Field name="startDate">
                    {(field) => (
                      <Win11DatePicker
                        value={field.state.value ? parse(field.state.value, 'yyyy-MM-dd', new Date()) : undefined}
                        onChange={(date) => field.handleChange(date ? format(date, 'yyyy-MM-dd') : '')}
                      />
                    )}
                  </form.Field>
                </div>
                <span className="shrink-0" style={{ color: 'var(--text-faint)' }}>–</span>
                <div className="flex-1 min-w-0">
                  <form.Field name="endDate">
                    {(field) => (
                      <Win11DatePicker
                        value={field.state.value ? parse(field.state.value, 'yyyy-MM-dd', new Date()) : undefined}
                        onChange={(date) => field.handleChange(date ? format(date, 'yyyy-MM-dd') : '')}
                      />
                    )}
                  </form.Field>
                </div>
              </div>
            </div>

            <div className="space-y-1.5">
              <span className="text-[11px] font-semibold pl-0.5 block invisible select-none" aria-hidden="true">
                占位
              </span>
              <div className="h-8 flex items-center gap-2">
                <div
                  className="w-8 h-4 rounded-full relative cursor-pointer shrink-0"
                  style={{
                    background: 'linear-gradient(135deg, var(--blue-primary), color-mix(in oklch, var(--blue-primary) 70%, #7c3aed))',
                    boxShadow: '0 2px 6px color-mix(in oklch, var(--blue-primary) 40%, transparent)',
                  }}
                >
                  <div className="w-3 h-3 bg-white rounded-full absolute top-[2px] left-[18px] shadow-sm" />
                </div>
                <span className="text-[13px] font-medium" style={{ color: 'var(--text-secondary)' }}>自动实时刷新</span>
              </div>
            </div>
          </div>
        </form>
      </div>

      {/* 5. 数据表格 */}
      <div
        className="flex flex-col flex-1 min-h-0 overflow-hidden"
        style={glassCard}
      >
        {/* 顶部高光 */}
        <div
          className="absolute top-0 left-0 right-0 h-px"
          style={{ background: 'linear-gradient(90deg, transparent, rgba(255,255,255,0.20), transparent)' }}
        />

        <div
          className="px-4 py-3 flex justify-between items-center shrink-0 border-b"
          style={{ borderColor: 'var(--table-border)' }}
        >
          <div className="flex items-center gap-2">
            <h3 className="text-[14px] font-bold" style={{ color: 'var(--text-primary)' }}>
              近期流水
            </h3>
            <span
              className="text-[11px] font-semibold px-2 py-0.5 rounded-full tabular-nums"
              style={{ background: 'var(--blue-pale-2)', color: 'var(--blue-light)' }}
            >
              Top 6
            </span>
          </div>
          <button
            className="w-7 h-7 rounded-lg flex items-center justify-center transition-all"
            style={{
              background: 'var(--glass-bg)',
              backdropFilter: 'blur(8px)',
              WebkitBackdropFilter: 'blur(8px)',
              border: '1px solid var(--glass-border)',
              color: 'var(--text-muted)',
            }}
            onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--glass-bg-strong)')}
            onMouseLeave={(e) => (e.currentTarget.style.background = 'var(--glass-bg)')}
          >
            <Settings weight="duotone" className="w-3.5 h-3.5" />
          </button>
        </div>

        <div className="flex-1 overflow-x-auto overflow-y-auto min-h-0 thin-scrollbar">
          <table className="w-full text-left border-collapse">
            <thead
              className="sticky top-0 z-10"
              style={{
                background: 'var(--table-header-bg)',
                backdropFilter: 'blur(12px)',
                WebkitBackdropFilter: 'blur(12px)',
              }}
            >
              {table.getHeaderGroups().map(headerGroup => (
                <tr
                  key={headerGroup.id}
                  className="text-[11px] font-semibold uppercase tracking-widest border-b"
                  style={{ color: 'var(--table-header-text)', borderColor: 'var(--table-border)' }}
                >
                  {headerGroup.headers.map(header => (
                    <th
                      key={header.id}
                      className={(header.column.columnDef.meta as any)?.className || "px-3 py-2.5"}
                    >
                      {header.isPlaceholder ? null : flexRender(header.column.columnDef.header, header.getContext())}
                    </th>
                  ))}
                </tr>
              ))}
            </thead>
            <tbody className="text-[13px]" style={{ color: 'var(--text-secondary)' }}>
              {isLoading ? (
                Array.from({ length: 6 }).map((_, idx) => (
                  <tr key={`sk-${idx}`} className="border-b" style={{ borderColor: 'var(--table-border)' }}>
                    {Array.from({ length: 8 }).map((_, i) => (
                      <td key={i} className="px-3 py-3">
                        <div
                          className="h-4 rounded-md animate-pulse"
                          style={{
                            background: 'var(--glass-bg-strong)',
                            width: `${[14, 24, 32, 40, 20, 16, 12, 16][i] * 4}px`,
                          }}
                        />
                      </td>
                    ))}
                  </tr>
                ))
              ) : table.getRowModel().rows.length === 0 ? (
                <tr>
                  <td colSpan={8} className="px-5 py-10 text-center text-[13px]" style={{ color: 'var(--text-muted)' }}>
                    暂无数据
                  </td>
                </tr>
              ) : (
                table.getRowModel().rows.map((row) => (
                  <tr
                    key={row.id}
                    className="border-b transition-all duration-150 cursor-default"
                    style={{ borderColor: 'var(--table-border)' }}
                    onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--table-row-hover)')}
                    onMouseLeave={(e) => (e.currentTarget.style.background = 'transparent')}
                  >
                    {row.getVisibleCells().map(cell => (
                      <td
                        key={cell.id}
                        className={(cell.column.columnDef.meta as any)?.className || "px-3 py-2.5"}
                      >
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </td>
                    ))}
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  )
}
