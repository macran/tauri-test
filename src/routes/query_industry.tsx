import { createFileRoute } from '@tanstack/react-router'
import { IndustryQuery } from '../features/query/QueryGroups'

export const Route = createFileRoute('/query_industry')({
  component: IndustryQuery,
})
