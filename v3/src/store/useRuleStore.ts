import { create } from 'zustand'
import type { TaxRule, RuleCategory } from '../components/RuleEngine'

const INITIAL_RULES: TaxRule[] = [
  {
    id: 'r-1',
    code: 'RULE-VAT-001',
    name: '福利费与餐费发票不可抵扣转出',
    category: 'vat',
    priority: 1,
    enabled: true,
    logicOperator: 'OR',
    conditions: [
      { id: 'c1', field: 'summary', operator: 'contains', value: '餐费' },
      { id: 'c2', field: 'summary', operator: 'contains', value: '员工福利' },
      { id: 'c3', field: 'ap_type', operator: 'equals', value: 'WELFARE_AP' },
    ],
    action: { vatTransfer: true, vatReason: '集体福利与个人消费，按规定进项转出' },
    leAppliesTo: ['ALL'],
    updatedBy: '张三',
    updatedAt: '2026-07-28 14:20',
  },
  {
    id: 'r-2',
    code: 'RULE-VAT-002',
    name: 'IC 关联方非应税业务转出',
    category: 'vat',
    priority: 2,
    enabled: true,
    logicOperator: 'AND',
    conditions: [
      { id: 'c4', field: 'ap_type', operator: 'equals', value: 'IC_TRANSFER' },
      { id: 'c5', field: 'tax_code', operator: 'equals', value: '3040100000000000000' },
    ],
    action: { vatTransfer: true, vatReason: '关联方非应税服务划转' },
    leAppliesTo: ['1001', '1002'],
    updatedBy: '李四',
    updatedAt: '2026-07-30 09:15',
  },
  {
    id: 'r-3',
    code: 'RULE-STAMP-001',
    name: '买卖与采购合同印花税计提 (0.03%)',
    category: 'stamp',
    priority: 1,
    enabled: true,
    logicOperator: 'OR',
    conditions: [
      { id: 'c6', field: 'summary', operator: 'contains', value: '设备采购' },
      { id: 'c7', field: 'summary', operator: 'contains', value: '货物买卖' },
    ],
    action: { stampDutyApplies: true, stampRate: 0.0003, stampCategory: '买卖合同' },
    leAppliesTo: ['ALL'],
    updatedBy: '张三',
    updatedAt: '2026-07-25 11:00',
  },
  {
    id: 'r-4',
    code: 'RULE-STAMP-002',
    name: '借款与融资租赁合同印花税 (0.005%)',
    category: 'stamp',
    priority: 2,
    enabled: false,
    logicOperator: 'AND',
    conditions: [
      { id: 'c8', field: 'summary', operator: 'contains', value: '银行借款利息' },
    ],
    action: { stampDutyApplies: true, stampRate: 0.00005, stampCategory: '借款合同' },
    leAppliesTo: ['1001'],
    updatedBy: '王五',
    updatedAt: '2026-07-15 16:40',
  },
]

interface RuleState {
  rules: TaxRule[]
  activeCategory: RuleCategory
  searchQuery: string
  statusFilter: 'all' | 'enabled' | 'disabled'
  isEditorOpen: boolean
  editingRule: TaxRule | null

  setActiveCategory: (c: RuleCategory) => void
  setSearchQuery: (q: string) => void
  setStatusFilter: (f: 'all' | 'enabled' | 'disabled') => void
  toggleEnabled: (id: string) => void
  deleteRule: (id: string) => void
  movePriority: (id: string, dir: 'up' | 'down') => void
  openEditor: (rule?: TaxRule) => void
  closeEditor: () => void
  saveRule: (rule: TaxRule) => void
}

export const useRuleStore = create<RuleState>()((set, get) => ({
  rules: INITIAL_RULES,
  activeCategory: 'vat',
  searchQuery: '',
  statusFilter: 'all',
  isEditorOpen: false,
  editingRule: null,

  setActiveCategory: (c) => set({ activeCategory: c }),
  setSearchQuery: (q) => set({ searchQuery: q }),
  setStatusFilter: (f) => set({ statusFilter: f }),

  toggleEnabled: (id) =>
    set((s) => ({
      rules: s.rules.map((r) =>
        r.id === id ? { ...r, enabled: !r.enabled, updatedAt: '刚刚' } : r
      ),
    })),

  deleteRule: (id) =>
    set((s) => ({ rules: s.rules.filter((r) => r.id !== id) })),

  movePriority: (id, dir) => {
    const { rules, activeCategory } = get()
    const cat = rules
      .filter((r) => r.category === activeCategory)
      .sort((a, b) => a.priority - b.priority)
    const idx = cat.findIndex((r) => r.id === id)
    if (idx === -1) return
    if (dir === 'up' && idx === 0) return
    if (dir === 'down' && idx === cat.length - 1) return
    const target = cat[dir === 'up' ? idx - 1 : idx + 1]
    const current = cat[idx]
    set({
      rules: rules.map((r) => {
        if (r.id === current.id) return { ...r, priority: target.priority }
        if (r.id === target.id) return { ...r, priority: current.priority }
        return r
      }),
    })
  },

  openEditor: (rule) => {
    const { rules, activeCategory } = get()
    if (rule) {
      set({ editingRule: JSON.parse(JSON.stringify(rule)), isEditorOpen: true })
    } else {
      const nextPriority = rules.filter((r) => r.category === activeCategory).length + 1
      set({
        editingRule: {
          id: `r-${Date.now()}`,
          code: `RULE-${activeCategory.toUpperCase()}-${String(nextPriority).padStart(3, '0')}`,
          name: '',
          category: activeCategory,
          priority: nextPriority,
          enabled: true,
          logicOperator: 'OR',
          conditions: [{ id: `c-${Date.now()}`, field: 'summary', operator: 'contains', value: '' }],
          action:
            activeCategory === 'vat'
              ? { vatTransfer: true, vatReason: '' }
              : { stampDutyApplies: true, stampRate: 0.0003, stampCategory: '买卖合同' },
          leAppliesTo: ['ALL'],
          updatedBy: '当前操作员',
          updatedAt: '刚刚',
        },
        isEditorOpen: true,
      })
    }
  },

  closeEditor: () => set({ isEditorOpen: false, editingRule: null }),

  saveRule: (rule) => {
    set((s) => {
      const exists = s.rules.some((r) => r.id === rule.id)
      return {
        rules: exists ? s.rules.map((r) => (r.id === rule.id ? rule : r)) : [...s.rules, rule],
        isEditorOpen: false,
        editingRule: null,
      }
    })
  },
}))
