import { createFileRoute } from '@tanstack/react-router'
import { VoucherQuery } from '../features/query/QueryGroups'

export const Route = createFileRoute('/query_voucher')({
  component: VoucherQuery,
})
