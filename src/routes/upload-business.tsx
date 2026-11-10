import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/upload-business')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>Hello "/upload-business"!</div>
}
