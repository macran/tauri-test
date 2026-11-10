import { createFileRoute } from '@tanstack/react-router'
import { InvoiceQuery } from '../features/query/QueryGroups'

export const Route = createFileRoute('/query_invoice')({
  component: InvoiceQuery,
})
