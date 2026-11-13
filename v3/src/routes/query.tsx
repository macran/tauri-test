import { createFileRoute } from '@tanstack/react-router'
import { OverviewQuery } from '../features/query/QueryGroups'

export const Route = createFileRoute('/query')({
  component: OverviewQuery,
})
