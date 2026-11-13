import { createFileRoute } from '@tanstack/react-router'
import StampTaxPage from '../features/stamp-tax/StampTaxPage'

export const Route = createFileRoute('/stamp_tax')({
  component: StampTaxPage,
})
