"use client"

import * as React from "react"
import { format, isValid, parse } from "date-fns"
import { zhCN } from "date-fns/locale"
import type { Locale } from "date-fns"
import { CalendarDays, X } from "lucide-react"
import type { Matcher } from "react-day-picker"

import { cn } from "@/lib/utils"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import {
  Win11Calendar,
  WIN11_DATE_FORMATS,
  normalizeByPrecision,
  type Win11Precision,
} from "@/components/ui/win11-calendar"

/* ═══════════════════ 受控 / 非受控双模 ═══════════════════ */

function useControllableState<T>({
  value,
  defaultValue,
  onChange,
  controlled,
}: {
  value: T | undefined
  defaultValue?: T
  onChange?: (next: T | undefined) => void
  controlled: boolean
}) {
  const [uncontrolledValue, setUncontrolledValue] = React.useState<
    T | undefined
  >(defaultValue)

  // 受控状态只在组件首次渲染时确定。
  // 后续即使 value 从 Date 变成 undefined，也不能改变受控/非受控模式。
  const isControlledRef = React.useRef(controlled)
  const isControlled = isControlledRef.current

  const currentValue = isControlled ? value : uncontrolledValue

  const onChangeRef = React.useRef(onChange)

  React.useEffect(() => {
    onChangeRef.current = onChange
  }, [onChange])

  const setValue = React.useCallback(
    (next: T | undefined) => {
      if (!isControlled) {
        setUncontrolledValue(next)
      }

      onChangeRef.current?.(next)
    },
    [isControlled],
  )

  return [currentValue, setValue] as const
}

/* ═══════════════════ 文本解析 ═══════════════════ */

/** 严格解析：回写后与原文完全一致才接受，拒绝 date-fns 的宽松补全 */
function parseStrict(text: string, pattern: string): Date | undefined {
  const trimmed = text.trim()
  if (!trimmed) return undefined
  const parsed = parse(trimmed, pattern, new Date())
  if (!isValid(parsed)) return undefined
  if (format(parsed, pattern) !== trimmed) return undefined
  return parsed
}

/* ═══════════════════ Props ═══════════════════ */

type InputPassthrough = Omit<
  React.ComponentPropsWithoutRef<"input">,
  "value" | "defaultValue" | "onChange" | "onBlur" | "type" | "placeholder"
>

export interface Win11DatePickerProps extends InputPassthrough {
  value?: Date
  defaultValue?: Date
  onChange?: (date: Date | undefined) => void
  onBlur?: React.FocusEventHandler<HTMLInputElement>

  /** "day" → yyyy-MM-dd；"month" → yyyy-MM（所属期间） */
  precision?: Win11Precision
  /** 覆盖默认的输入与显示格式 */
  dateFormat?: string
  /** 透传给日视图，默认不禁用任何日期 */
  disabledDates?: Matcher | Matcher[]

  locale?: Locale
  weekStartsOn?: 0 | 1

  /** 有值时显示清除按钮，默认 true */
  clearable?: boolean
  /** 默认不显示占位文字；仅在确有必要时传入 */
  placeholder?: string
  align?: "start" | "center" | "end"
  side?: "top" | "right" | "bottom" | "left"

  className?: string
  inputClassName?: string
  panelClassName?: string
}

/* ═══════════════════ 组件 ═══════════════════ */

export const Win11DatePicker = React.forwardRef<
  HTMLInputElement,
  Win11DatePickerProps
>(function Win11DatePicker(props, forwardedRef) {
  // 受控与否看"传没传 value 这个 prop"，而不是 value 是否为 undefined：
  // 查询表单的空日期就是 undefined，按值判断会把它误当成非受控，form.reset() 就清不掉。
  const controlled = "value" in props
  const {
    value: valueProp,
    defaultValue,
    onChange,
    onBlur,
    precision = "day",
    dateFormat,
    disabledDates,
    locale = zhCN,
    weekStartsOn,
    clearable = true,
    placeholder,
    align = "start",
    side = "bottom",
    disabled,
    className,
    inputClassName,
    panelClassName,
    "aria-invalid": ariaInvalid,
    "aria-describedby": ariaDescribedBy,
    id,
    name,
    ...inputProps
  } = props
  const pattern = dateFormat ?? WIN11_DATE_FORMATS[precision]

  const [value, setValue] = useControllableState<Date | undefined>({
    value: valueProp,
    defaultValue,
    onChange,
    controlled,
  })

  const [open, setOpen] = React.useState(false)
  const [text, setText] = React.useState(() =>
    value ? format(value, pattern) : "",
  )
  const [textInvalid, setTextInvalid] = React.useState(false)

  const inputRef = React.useRef<HTMLInputElement>(null)
  React.useImperativeHandle(forwardedRef, () => inputRef.current!, [])
  const returnFocusRef = React.useRef(false)
  const hintId = React.useId()

  /* 值变化 → 回写文本。文本的唯一写入点（除用户输入外），
     避免 onSelect 与此处双写造成闪烁 */
  React.useEffect(() => {
    setText(value ? format(value, pattern) : "")
    setTextInvalid(false)
  }, [value, pattern])

  /* ───────── 提交与清除 ───────── */

  function commit(next: Date | undefined) {
    setValue(next ? normalizeByPrecision(next, precision) : undefined)
  }

  /** 幂等：内容未变则不触碰任何 state，避免空转重渲染与无意义的外部 onChange */
  function commitText() {
    const trimmed = text.trim()

    if (!trimmed) {
      if (textInvalid) setTextInvalid(false)
      if (value) commit(undefined)
      else if (text !== "") setText("")
      return
    }

    const parsed = parseStrict(trimmed, pattern)
    if (!parsed) {
      if (!textInvalid) setTextInvalid(true)
      return
    }

    const normalized = format(parsed, pattern)
    if (textInvalid) setTextInvalid(false)
    if (normalized !== text) setText(normalized)
    // 值未变就不提交 —— 否则查询表单会白跑一次 navigate + SQLite 查询
    if (!value || format(value, pattern) !== normalized) commit(parsed)
  }

  function clear() {
    setText("")
    setTextInvalid(false)
    commit(undefined)

    requestAnimationFrame(() => {
      inputRef.current?.focus()
    })
  }

  /* ───────── 输入框事件 ───────── */

  function handleTextChange(e: React.ChangeEvent<HTMLInputElement>) {
    setText(e.target.value)
    setTextInvalid(false)
    if (!open) setOpen(true)
    // 输入过程不向外提交；提交时机统一在 blur / Enter / 日历点击
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter") {
      e.preventDefault()
      commitText()
      returnFocusRef.current = true
      setOpen(false)
      return
    }
    if (e.key === "Escape") {
      // 两段式：面板开着先关面板，关着则清值
      if (open) {
        setOpen(false)
        return
      }
      if (clearable && text) {
        e.preventDefault()
        clear()
      }
      return
    }
    if ((e.key === "ArrowDown" || e.key === "ArrowUp") && !open) {
      e.preventDefault()
      setOpen(true)
    }
  }

  function handleBlur(e: React.FocusEvent<HTMLInputElement>) {
    /* 焦点落在面板或清除按钮上时不提交。
       Popover 走 portal，closest 必须按 data-slot 判断而非 contains */
    const next = e.relatedTarget as HTMLElement | null
    if (
      next?.closest("[data-slot='win11-date-picker-panel']") ||
      next?.closest("[data-slot='win11-date-picker']")
    ) {
      return
    }
    commitText()
    onBlur?.(e)
  }

  /* ───────── 面板开合 ───────── */

  function handleOpenChange(next: boolean) {
    if (next) {
      // 打开时用已输入的合法文本定位面板，哪怕尚未提交
      const typed = parseStrict(text, pattern)
      if (
        typed &&
        (!value || format(value, pattern) !== format(typed, pattern))
      ) {
        commit(typed)
      }
    } else {
      commitText()
      returnFocusRef.current = false
    }
    setOpen(next)
  }

  /** 面板定位值：优先已提交值，其次正在输入的合法文本 */
  const panelValue = value ?? parseStrict(text, pattern)

  const invalid =
    ariaInvalid === true || ariaInvalid === "true" || textInvalid
  const showClear = clearable && !!text && !disabled

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger
        nativeButton={false}
        render={
          <div
            data-slot="win11-date-picker"
            className={cn("relative flex h-(--control-h) w-full items-center", precision === "month" ? "min-w-[104px]" : "min-w-[132px]", className)}
          />
        }
      >
        <input
          {...inputProps}
          ref={inputRef}
          id={id}
          name={name}
          type="text"
          inputMode="numeric"
          autoComplete="off"
          data-slot="win11-date-picker-input"
          value={text}
          onChange={handleTextChange}
          onKeyDown={handleKeyDown}
          onBlur={handleBlur}
          onMouseDown={(e) => {
            e.stopPropagation()
            setOpen(true)
          }}
          onClick={(e) => e.stopPropagation()}
          disabled={disabled}
          placeholder={placeholder}
          title={`格式 ${pattern}`}
          aria-invalid={invalid || undefined}
          aria-haspopup="dialog"
          aria-expanded={open}
          aria-describedby={cn(ariaDescribedBy, hintId)}
          className={cn(
            "h-(--control-h) w-full rounded-md px-3 pr-9 text-sm outline-none",
            // 字段态：浅填充 + 可见边框 + 极淡内阴影，在白底容器上也能"凹"出来
            "border border-[var(--w11-field-border)] bg-[var(--w11-field-bg)]",
            "text-foreground placeholder:text-muted-foreground",
            "shadow-[inset_0_1px_0_rgb(0_0_0/0.02)]",
            "transition-[border-color,box-shadow] duration-150",
            "motion-reduce:transition-none",
            // 交互态逐级增强
            "hover:border-[var(--w11-field-border-hover)]",
            "focus:border-[var(--w11-accent)]",
            // Win11 文本框聚焦特征：底部一道强调色线
            "focus:shadow-[inset_0_-2px_0_var(--w11-accent)]",
            "disabled:cursor-not-allowed disabled:bg-muted/50 disabled:opacity-60",
            "disabled:hover:border-[var(--w11-field-border)]",
            invalid &&
            cn(
              "border-destructive bg-destructive/[0.03]",
              "hover:border-destructive",
              "focus:border-destructive",
              "focus:shadow-[inset_0_-2px_0_var(--destructive)]",
            ),
            inputClassName,
          )}
        />

        <span id={hintId} className="sr-only">
          可直接输入，格式 {pattern}
        </span>

        {showClear ? (
          <button
            type="button"
            aria-label="清除日期"
            onPointerDown={(event) => {
              // 阻止 pointerdown 导致 input 失焦
              event.preventDefault()
              event.stopPropagation()
            }}
            onClick={(event) => {
              event.preventDefault()
              event.stopPropagation()
              clear()
            }}
            className={cn(
              "absolute right-2 flex size-5 shrink-0 items-center justify-center",
              "rounded-full text-muted-foreground outline-none",
              "transition-colors duration-150 motion-reduce:transition-none",
              "hover:bg-[var(--w11-accent)] hover:text-[var(--w11-accent-fg)]",

            )}
          >
            <X className="size-3" aria-hidden="true" />
          </button>
        ) : (
          <CalendarDays
            className="pointer-events-none absolute right-3 size-4 shrink-0 text-muted-foreground"
            aria-hidden="true"
          />
        )}
      </PopoverTrigger>

      <PopoverContent
        align={align}
        side={side}
        initialFocus={false}
        /* 关键修复：阻止点击引起的焦点转移。
           input 保持聚焦 → 不触发 blur → 不 commit → 不重建网格 →
           mouseup 时目标节点仍在，click 正常派发。
           preventDefault 不影响 click 事件本身，
           也不影响键盘触发的 :focus-visible */
        onMouseDown={(e) => e.preventDefault()}
        data-slot="win11-date-picker-panel"
        className={cn(
          // 254 = 7 × 34 + 16，格子宽度为整数，高 DPI 下边界不虚
          "w-[254px] overflow-hidden border-border/70 bg-[var(--w11-panel-bg)] p-0",
          "shadow-xl backdrop-blur-md",
          panelClassName,
        )}
      >
        <Win11Calendar
          value={panelValue}
          precision={precision}
          disabledDates={disabledDates}
          locale={locale}
          weekStartsOn={weekStartsOn}
          onSelect={(picked) => {
            returnFocusRef.current = true
            commit(picked) // 文本由 useEffect 单一回写
            setOpen(false)
          }}
        />
      </PopoverContent>
    </Popover>
  )
})

/* ═══════════════════ 期间选择器 ═══════════════════ */

export const Win11MonthPicker = React.forwardRef<
  HTMLInputElement,
  Omit<Win11DatePickerProps, "precision">
>(function Win11MonthPicker(props, ref) {
  return <Win11DatePicker {...props} ref={ref} precision="month" />
})