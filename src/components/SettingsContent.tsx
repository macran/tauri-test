import { useForm } from '@tanstack/react-form'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'

export default function SettingsContent() {
  const form = useForm({
    defaultValues: {
      theme: 'light',
      compactMode: false,
      animations: true,
    },
    onSubmit: async ({ value }) => {
      console.log('Saved settings:', value)
    }
  })

  return (
    <div className="p-6 bg-white dark:bg-slate-900 rounded-xl border border-slate-200 dark:border-slate-800 shadow-sm max-w-2xl">
      <h2 className="text-lg font-bold text-slate-800 dark:text-slate-100 mb-6">偏好与设置 (Layout & Settings)</h2>
      
      <form
        onSubmit={(e) => {
          e.preventDefault()
          e.stopPropagation()
          form.handleSubmit()
        }}
        className="space-y-6"
      >
        <form.Field name="compactMode">
          {(field) => (
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label className="text-sm font-medium">紧凑模式 (Compact Mode)</Label>
                <p className="text-xs text-slate-500">减少表格和列表的空白间距，显示更多内容</p>
              </div>
              <Switch
                checked={field.state.value}
                onCheckedChange={field.handleChange}
              />
            </div>
          )}
        </form.Field>

        <form.Field name="animations">
          {(field) => (
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label className="text-sm font-medium">动画效果 (Animations)</Label>
                <p className="text-xs text-slate-500">开启或关闭界面过渡动画</p>
              </div>
              <Switch
                checked={field.state.value}
                onCheckedChange={field.handleChange}
              />
            </div>
          )}
        </form.Field>

        <div className="pt-4 flex justify-end">
          <Button type="submit" className="bg-sky-500 hover:bg-sky-600 text-white">
            保存设置
          </Button>
        </div>
      </form>
    </div>
  )
}
