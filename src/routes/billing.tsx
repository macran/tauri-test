import { createFileRoute } from '@tanstack/react-router'
import { ReversalManage } from '../features/tax-period/TaxManagePages'

export const Route = createFileRoute('/billing')({
  component: ReversalManage,
})
