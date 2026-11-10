import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/upload-media')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>Hello "/upload-media"!</div>
}
