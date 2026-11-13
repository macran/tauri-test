import React from "react"
import { IconProps } from "@phosphor-icons/react"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select"

// ── Card ──────────────────────────────────────────────────────────────────────
interface LiquidCardProps {
  title?: string
  subtitle?: string
  icon?: React.ComponentType<IconProps>
  badge?: string
  children: React.ReactNode
  className?: string
  noPadding?: boolean
}

export function LiquidCard({
  title,
  subtitle,
  icon: Icon,
  badge,
  children,
  className = "",
  noPadding = false,
}: LiquidCardProps) {
  return (
    <div
      className={`rounded-xl border transition-all duration-200 ${noPadding ? '' : 'p-4'} ${className}`}
      style={{
        backgroundColor: 'var(--card-bg)',
        borderColor: 'var(--card-border)',
        boxShadow: 'var(--card-shadow)',
      }}
    >
      {(title || Icon) && (
        <div className={`flex items-center justify-between ${noPadding ? 'px-4 pt-3 pb-3 border-b' : 'mb-3'}`}
          style={noPadding ? { borderColor: 'var(--card-border)' } : {}}>
          <div className="flex items-center gap-2.5">
            {Icon && (
              <div
                className="p-1.5 rounded-lg"
                style={{ backgroundColor: 'var(--blue-pale)', color: 'var(--blue-primary)' }}
              >
                <Icon weight="duotone" className="w-4 h-4" />
              </div>
            )}
            <div>
              {title && (
                <h3 className="text-[13px] font-bold" style={{ color: 'var(--text-primary)' }}>
                  {title}
                </h3>
              )}
              {subtitle && (
                <p className="text-[11px]" style={{ color: 'var(--text-muted)' }}>
                  {subtitle}
                </p>
              )}
            </div>
          </div>
          {badge && (
            <span
              className="px-2 py-0.5 text-[10px] font-semibold tracking-wide rounded-full"
              style={{
                backgroundColor: 'var(--blue-pale)',
                color: 'var(--blue-primary)',
              }}
            >
              {badge}
            </span>
          )}
        </div>
      )}
      <div>{children}</div>
    </div>
  )
}

// ── Input ─────────────────────────────────────────────────────────────────────
interface LiquidInputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  icon?: React.ComponentType<IconProps>
  label?: string
  clearable?: boolean
  onClear?: () => void
}

export function LiquidInput({
  icon: Icon,
  label,
  clearable,
  onClear,
  className = "",
  value,
  ...props
}: LiquidInputProps) {
  return (
    <div className="space-y-1 w-full">
      {label && (
        <label className="text-[11px] font-semibold pl-0.5 block" style={{ color: 'var(--text-muted)' }}>
          {label}
        </label>
      )}
      <div className="relative flex items-center">
        {Icon && (
          <Icon
            weight="regular"
            className="absolute left-2.5 w-3.5 h-3.5 pointer-events-none z-10"
            style={{ color: 'var(--text-faint)' }}
          />
        )}
        <input
          value={value}
          className={`
            w-full text-[13px] rounded-lg border outline-none transition-all duration-150
            ${Icon ? 'pl-8' : 'pl-3'}
            ${clearable && value ? 'pr-8' : 'pr-3'}
            py-1.5
            ${className}
          `}
          style={{
            backgroundColor: 'var(--input-bg)',
            borderColor: 'var(--input-border)',
            color: 'var(--input-text)',
          }}
          onFocus={(e) => {
            e.currentTarget.style.borderColor = 'var(--input-focus-ring)'
            e.currentTarget.style.boxShadow = '0 0 0 3px rgba(59,130,246,0.12)'
          }}
          onBlur={(e) => {
            e.currentTarget.style.borderColor = 'var(--input-border)'
            e.currentTarget.style.boxShadow = 'none'
          }}
          {...props}
        />
        {clearable && value && (
          <button
            type="button"
            onClick={onClear}
            className="absolute right-2 p-1 rounded transition-colors"
            style={{ color: 'var(--text-muted)' }}
          >
            ✕
          </button>
        )}
      </div>
    </div>
  )
}

// ── Select ────────────────────────────────────────────────────────────────────
interface LiquidSelectProps {
  icon?: React.ComponentType<IconProps>
  label?: string
  className?: string
  value?: string
  defaultValue?: string
  onChange?: (event: { target: { value: string } }) => void
  disabled?: boolean
  children: React.ReactNode
}

export function LiquidSelect({
  icon: Icon,
  label,
  className = "",
  value,
  defaultValue,
  onChange,
  disabled,
  children,
}: LiquidSelectProps) {
  const options = React.Children.toArray(children).filter(
    (child): child is React.ReactElement<{ value: string; children?: React.ReactNode; disabled?: boolean }> =>
      React.isValidElement(child)
  )
  const items = Object.fromEntries(options.map((option) => [option.props.value, option.props.children]))

  return (
    <div className="space-y-1 w-full">
      {label && (
        <label className="text-[11px] font-semibold pl-0.5 block" style={{ color: 'var(--text-muted)' }}>
          {label}
        </label>
      )}
      <Select
        items={items}
        value={value}
        defaultValue={defaultValue}
        disabled={disabled}
        onValueChange={(nextValue) => onChange?.({ target: { value: nextValue as string } })}
      >
        <div className="relative flex items-center w-full">
          {Icon && (
            <Icon
              weight="regular"
              className="absolute left-2.5 w-3.5 h-3.5 pointer-events-none z-10"
              style={{ color: 'var(--text-faint)' }}
            />
          )}
          <SelectTrigger
            className={`
              w-full text-[13px] rounded-lg border transition-all duration-150
              ${Icon ? 'pl-8' : 'pl-3'}
              pr-3 py-1.5 h-auto
              ${className}
            `}
            style={{
              backgroundColor: 'var(--input-bg)',
              borderColor: 'var(--input-border)',
              color: 'var(--input-text)',
            }}
          >
            <SelectValue />
          </SelectTrigger>
        </div>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.props.value} value={option.props.value} disabled={option.props.disabled}>
              {option.props.children}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  )
}

// ── Button ────────────────────────────────────────────────────────────────────
interface LiquidButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger" | "success"
  icon?: React.ComponentType<IconProps>
  iconPosition?: "left" | "right"
  children: React.ReactNode
}

export function LiquidButton({
  variant = "secondary",
  icon: Icon,
  iconPosition = "left",
  children,
  className = "",
  style,
  ...props
}: LiquidButtonProps) {
  const variantStyles: Record<string, React.CSSProperties> = {
    primary: {
      backgroundColor: 'var(--blue-primary)',
      color: '#ffffff',
      border: '1px solid var(--blue-primary)',
    },
    secondary: {
      backgroundColor: 'var(--input-bg)',
      color: 'var(--text-secondary)',
      border: '1px solid var(--input-border)',
    },
    ghost: {
      backgroundColor: 'transparent',
      color: 'var(--text-secondary)',
      border: '1px solid transparent',
    },
    danger: {
      backgroundColor: '#ef4444',
      color: '#ffffff',
      border: '1px solid #ef4444',
    },
    success: {
      backgroundColor: '#22c55e',
      color: '#ffffff',
      border: '1px solid #22c55e',
    },
  }

  return (
    <button
      className={`inline-flex items-center justify-center gap-1.5 px-3 py-1.5 text-[13px] font-medium rounded-lg transition-all duration-150 active:scale-95 select-none ${className}`}
      style={{ ...variantStyles[variant], ...style }}
      onMouseEnter={(e) => {
        const el = e.currentTarget
        if (variant === 'primary') el.style.opacity = '0.88'
        if (variant === 'secondary') el.style.backgroundColor = 'var(--blue-pale)'
        if (variant === 'ghost') el.style.backgroundColor = 'var(--blue-pale)'
      }}
      onMouseLeave={(e) => {
        const el = e.currentTarget
        el.style.opacity = '1'
        el.style.backgroundColor = variantStyles[variant].backgroundColor as string
      }}
      {...props}
    >
      {Icon && iconPosition === "left" && <Icon weight="regular" className="w-3.5 h-3.5" />}
      <span>{children}</span>
      {Icon && iconPosition === "right" && <Icon weight="regular" className="w-3.5 h-3.5" />}
    </button>
  )
}
