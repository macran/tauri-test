import { createFileRoute } from '@tanstack/react-router'
import { CertifiedQuery } from '../features/query/QueryGroups'

export const Route = createFileRoute('/query_certified')({
  component: CertifiedQuery,
})
