import { createFileRoute } from '@tanstack/react-router'
import DashboardContent from '../components/DashboardContent'

export const Route = createFileRoute('/dashboard')({
  component: DashboardContent,
})
