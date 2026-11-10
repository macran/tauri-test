import { createFileRoute } from '@tanstack/react-router'
import TaxRulesPage from '../features/rules/TaxRulesPage'

export const Route = createFileRoute('/rules')({
  component: TaxRulesPage,
})
