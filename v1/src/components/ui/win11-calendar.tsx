"use client"

import * as React from "react"
import {
  addDays,
  addMonths,
  addYears,
  format,
  getMonth,
  getYear,
  isSameMonth,
  startOfDay,
  startOfMonth,
  startOfWeek,
} from "date-fns"
import { zhCN } from "date-fns/locale"
import type { Locale } from "date-fns"
import { ChevronDown, ChevronUp } from "lucide-react"
import { DayButton, type DayButtonProps, type Matcher } from "react-day-picker"

import { cn } from "@/lib/utils"
import { Calendar } from "@/components/ui/calendar"

/* ═══════════════════════ 类型与常量 ═══════════════════════ */

export type Win11Precision = "day" | "month"
export type Win11View = "day" | "month" | "year"

export const WIN11_DATE_FORMATS: Record<Win11Precision, string> = {
  day: "yyyy-MM-dd",
  month: "yyyy-MM",
}

const INITIAL_VIEW: Record<Win11Precision, Win11View> = {
  day: "day",
  month: "month",
}

/** 日/月/年三个网格统一高度，切换视图时面板不跳动 */
const GRID_HEIGHT = 252

type Transition =
  | { type: "zoom"; direction: "in" | "out" }
  | { type: "slide"; direction: "up" | "down" }

export function win11DecadeStart(year: number) {
  return Math.floor(year / 10) * 10
}

/** 按精度归一到日/月起点，避免时间分量污染查询条件 */
export function normalizeByPrecision(date: Date, precision: Win11Precision) {
  return precision === "month" ? startOfMonth(date) : startOfDay(date)
}

function clickMotion(className?: string) {
  return cn(
    "transition-transform duration-150 active:scale-95",
    "motion-reduce:transition-none motion-reduce:active:scale-100",
    className,
  )
}

/* ═══════════════════════ 日单元格 ═══════════════════════ */

function Win11DayButton({
  className,
  day,
  modifiers,
  children,
  ...props
}: DayButtonProps) {
  return (
    <DayButton
      {...props}
      day={day}
      modifiers={modifiers}
      className={clickMotion(
        cn(
          "relative flex h-[42px] w-full min-w-0 items-center justify-center",
          "rounded-md px-1 py-1 text-xs font-medium leading-none outline-none",
          // 圆形背景由 ::before 承担，单元格自身始终透明
          "before:pointer-events-none before:absolute before:left-1/2 before:top-1/2",
          "before:z-0 before:size-8 before:-translate-x-1/2 before:-translate-y-1/2",
          "before:rounded-full before:content-[''] before:opacity-0",
          "before:transition-opacity before:duration-150",
          "motion-reduce:before:transition-none",
          "text-foreground focus-visible:ring-2 focus-visible:ring-ring",
          "hover:before:bg-[var(--w11-accent)] hover:before:opacity-100",
          "hover:text-[var(--w11-accent-fg)]",
          modifiers.outside && "text-muted-foreground/45",
          modifiers.disabled && "pointer-events-none opacity-40",
          modifiers.today &&
            !modifiers.selected &&
            cn(
              "text-[var(--w11-accent)]",
              "before:border-2 before:border-[var(--w11-accent)]",
              "before:bg-transparent before:opacity-100",
              "hover:before:bg-[var(--w11-accent)] hover:text-[var(--w11-accent-fg)]",
            ),
          modifiers.selected &&
            cn(
              "text-[var(--w11-accent-fg)]",
              "before:bg-[var(--w11-accent)] before:opacity-100",
              "hover:before:bg-[var(--w11-accent)]",
            ),
          className,
        ),
      )}
      aria-label={props["aria-label"] ?? format(day.date, "PPP", { locale: zhCN })}
    >
      <span className="relative z-10">{children ?? format(day.date, "d")}</span>
    </DayButton>
  )
}

/* 模块作用域常量：避免每次渲染生成新对象，
   否则 react-day-picker 会卸载重建整个日期网格 */
const CALENDAR_COMPONENTS = { DayButton: Win11DayButton } as const

/* react-day-picker 的 classNames 是替换而非合并，
   冲突项给空串即彻底清掉默认样式，无需 !important */
const CALENDAR_CLASSNAMES = {
  months: "w-full",
  month: "w-full gap-0",
  month_caption: "hidden",
  caption_label: "hidden",
  nav: "hidden",
  month_grid: "w-full border-collapse",
  weekdays: "hidden",
  week: "flex h-[42px] w-full",
  day: "min-w-0 flex-1 p-0",
  day_button: "h-[42px] w-full",
  today: "",
  selected: "",
  outside: "text-muted-foreground/45",
  disabled: "opacity-40",
  hidden: "invisible",
} as const

/* ═══════════════════════ 星期行 ═══════════════════════ */

function WeekdaysRow({
  locale,
  weekStartsOn,
}: {
  locale: Locale
  weekStartsOn?: 0 | 1
}) {
  const labels = React.useMemo(() => {
    const start = startOfWeek(new Date(), { locale, weekStartsOn })
    return Array.from({ length: 7 }, (_, i) =>
      format(addDays(start, i), "EEEEEE", { locale }),
    )
  }, [locale, weekStartsOn])

  return (
    <div className="flex h-8 w-full items-center px-2">
      {labels.map((label, i) => (
        <div
          key={i}
          className="flex flex-1 items-center justify-center text-[11px] font-medium text-muted-foreground"
        >
          {label}
        </div>
      ))}
    </div>
  )
}

/* ═══════════════════════ 面板头部 ═══════════════════════ */

interface HeaderProps {
  view: Win11View
  month: Date
  yearPageStart: number
  locale: Locale
  onTitleClick: () => void
  onPrevious: () => void
  onNext: () => void
}

function PickerHeader({
  view,
  month,
  yearPageStart,
  locale,
  onTitleClick,
  onPrevious,
  onNext,
}: HeaderProps) {
  const title =
    view === "day"
      ? format(month, "yyyy年M月", { locale })
      : view === "month"
        ? format(month, "yyyy年", { locale })
        : `${yearPageStart} - ${yearPageStart + 9}`

  const stepLabels: Record<Win11View, [string, string]> = {
    day: ["上一个月", "下一个月"],
    month: ["上一年", "下一年"],
    year: ["上一组十年", "下一组十年"],
  }
  const [prevLabel, nextLabel] = stepLabels[view]

  const titleLabel =
    view === "day"
      ? "切换到月份选择"
      : view === "month"
        ? "切换到年份选择"
        : "当前年份范围"

  return (
    <div className="flex h-10 items-center gap-2 px-2">
      <button
        type="button"
        onClick={onTitleClick}
        disabled={view === "year"}
        aria-label={titleLabel}
        aria-expanded={view !== "day"}
        className={clickMotion(
          cn(
            "flex min-w-0 flex-1 items-center justify-start rounded-md px-2 py-1.5",
            "text-left text-sm font-semibold tracking-tight text-foreground outline-none",
            "hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring",
            "disabled:cursor-default disabled:hover:bg-transparent",
          ),
        )}
      >
        {title}
      </button>

      <div className="flex shrink-0 items-center gap-1">
        <button
          type="button"
          onClick={onPrevious}
          aria-label={prevLabel}
          className={clickMotion(
            "inline-flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring",
          )}
        >
          <ChevronUp className="size-4" aria-hidden="true" />
        </button>
        <button
          type="button"
          onClick={onNext}
          aria-label={nextLabel}
          className={clickMotion(
            "inline-flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring",
          )}
        >
          <ChevronDown className="size-4" aria-hidden="true" />
        </button>
      </div>
    </div>
  )
}

/* ═══════════════════════ 网格单元样式 ═══════════════════════ */

function gridCellClass(opts: {
  dimmed?: boolean
  current?: boolean
  selected?: boolean
}) {
  const { dimmed, current, selected } = opts
  return clickMotion(
    cn(
      "flex size-12 items-center justify-center rounded-full text-xs font-medium",
      "text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring",
      "transition-colors duration-150 motion-reduce:transition-none",
      "hover:bg-[var(--w11-accent)] hover:text-[var(--w11-accent-fg)]",
      dimmed && "text-muted-foreground/50",
      current &&
        !selected &&
        "border-2 border-[var(--w11-accent)] text-[var(--w11-accent)]",
      selected &&
        "bg-[var(--w11-accent)] text-[var(--w11-accent-fg)] hover:bg-[var(--w11-accent)]",
    ),
  )
}

const GRID_WRAPPER =
  "grid grid-cols-4 grid-rows-[repeat(4,63px)] place-items-center px-2"

/* ═══════════════════════ 月份网格 ═══════════════════════ */

function MonthGrid({
  month,
  value,
  locale,
  onSelect,
}: {
  month: Date
  value?: Date
  locale: Locale
  onSelect: (date: Date) => void
}) {
  const year = getYear(month)
  const today = new Date()

  return (
    <div
      key={`months-${year}`}
      role="listbox"
      aria-label={`${year}年月份`}
      className={GRID_WRAPPER}
      style={{ height: GRID_HEIGHT }}
    >
      {Array.from({ length: 16 }, (_, i) => {
        const overflow = i >= 12
        const monthIndex = i % 12
        const cellYear = overflow ? year + 1 : year
        const date = new Date(cellYear, monthIndex, 1)
        const selected =
          value !== undefined &&
          getYear(value) === cellYear &&
          getMonth(value) === monthIndex

        return (
          <button
            key={`${cellYear}-${monthIndex}`}
            type="button"
            role="option"
            aria-selected={selected}
            aria-label={format(date, "yyyy年M月", { locale })}
            onClick={() => onSelect(date)}
            className={gridCellClass({
              dimmed: overflow,
              current: isSameMonth(date, today),
              selected,
            })}
          >
            {monthIndex + 1}月
          </button>
        )
      })}
    </div>
  )
}

/* ═══════════════════════ 年份网格 ═══════════════════════ */

function YearGrid({
  yearPageStart,
  value,
  onSelect,
}: {
  yearPageStart: number
  value?: Date
  onSelect: (year: number) => void
}) {
  const currentYear = getYear(new Date())
  // 3 + 10 + 3 = 16，主十年居中
  const years = Array.from({ length: 16 }, (_, i) => yearPageStart - 3 + i)

  return (
    <div
      key={`years-${yearPageStart}`}
      role="listbox"
      aria-label={`${yearPageStart}年至${yearPageStart + 9}年`}
      className={GRID_WRAPPER}
      style={{ height: GRID_HEIGHT }}
    >
      {years.map((year) => {
        const inDecade = year >= yearPageStart && year <= yearPageStart + 9
        const selected = value !== undefined && getYear(value) === year

        return (
          <button
            key={year}
            type="button"
            role="option"
            aria-selected={selected}
            aria-label={`${year}年`}
            onClick={() => onSelect(year)}
            className={gridCellClass({
              dimmed: !inDecade,
              current: year === currentYear,
              selected,
            })}
          >
            {year}
          </button>
        )
      })}
    </div>
  )
}

/* ═══════════════════════ 日历面板 ═══════════════════════ */

export interface Win11CalendarProps {
  value?: Date
  /** 提交选中值。precision="month" 时点击月份即提交 */
  onSelect: (date: Date) => void
  precision?: Win11Precision
  /** 透传给日视图的禁用匹配器，不传则不禁用任何日期 */
  disabledDates?: Matcher | Matcher[]
  locale?: Locale
  weekStartsOn?: 0 | 1
  className?: string
}

export function Win11Calendar({
  value,
  onSelect,
  precision = "day",
  disabledDates,
  locale = zhCN,
  weekStartsOn,
  className,
}: Win11CalendarProps) {
  const [view, setView] = React.useState<Win11View>(INITIAL_VIEW[precision])
  const [month, setMonth] = React.useState(value ?? new Date())
  const [yearPageStart, setYearPageStart] = React.useState(() =>
    win11DecadeStart(getYear(value ?? new Date())),
  )
  const [transition, setTransition] = React.useState<Transition>({
    type: "zoom",
    direction: "in",
  })

  /* 外部值变化时同步面板定位。
     等值时返回同一引用，React 跳过重渲染，
     动画容器 key 不变 —— 这是「首次点击不生效」的修复之一 */
  React.useEffect(() => {
    if (!value) return
    setMonth((prev) => (isSameMonth(prev, value) ? prev : value))
    setYearPageStart((prev) => {
      const next = win11DecadeStart(getYear(value))
      return prev === next ? prev : next
    })
  }, [value])

  function handleTitleClick() {
    if (view === "day") {
      setTransition({ type: "zoom", direction: "out" })
      setView("month")
      return
    }
    if (view === "month") {
      setTransition({ type: "zoom", direction: "out" })
      setYearPageStart(win11DecadeStart(getYear(month)))
      setView("year")
    }
    // year 已是最上层视图
  }

  function step(delta: 1 | -1) {
    setTransition({ type: "slide", direction: delta > 0 ? "up" : "down" })
    if (view === "day") setMonth((m) => addMonths(m, delta))
    else if (view === "month") setMonth((m) => addYears(m, delta))
    else setYearPageStart((y) => y + delta * 10)
  }

  function handleMonthSelect(next: Date) {
    // 期间模式：月份即终点
    if (precision === "month") {
      onSelect(startOfMonth(next))
      return
    }
    setTransition({ type: "zoom", direction: "in" })
    setMonth(next)
    setView("day")
  }

  function handleYearSelect(year: number) {
    setTransition({ type: "zoom", direction: "in" })
    setMonth(new Date(year, getMonth(month), 1))
    setView("month")
  }

  const animKey =
    view === "day"
      ? format(month, "yyyy-MM")
      : view === "month"
        ? String(getYear(month))
        : String(yearPageStart)

  return (
    <div data-slot="win11-calendar" className={className}>
      <PickerHeader
        view={view}
        month={month}
        yearPageStart={yearPageStart}
        locale={locale}
        onTitleClick={handleTitleClick}
        onPrevious={() => step(-1)}
        onNext={() => step(1)}
      />

      {view === "day" && (
        <WeekdaysRow locale={locale} weekStartsOn={weekStartsOn} />
      )}

      <div
        key={`${view}-${animKey}`}
        className={cn(
          "overflow-hidden",
          view === "day" && "-mt-2",
          "motion-safe:animate-in motion-safe:fade-in-0 motion-safe:duration-200",
          "motion-reduce:animate-none",
          transition.type === "zoom" &&
            (transition.direction === "out"
              ? "motion-safe:zoom-in-125"
              : "motion-safe:zoom-in-75"),
          transition.type === "slide" &&
            (transition.direction === "down"
              ? "motion-safe:slide-in-from-top-4"
              : "motion-safe:slide-in-from-bottom-4"),
        )}
      >
        {view === "day" ? (
          <Calendar
            mode="single"
            month={month}
            onMonthChange={setMonth}
            selected={value}
            onSelect={(next, triggerDate) => {
              const picked = next ?? triggerDate
              if (picked) onSelect(startOfDay(picked))
            }}
            disabled={disabledDates}
            locale={locale}
            weekStartsOn={weekStartsOn}
            showOutsideDays
            fixedWeeks
            hideNavigation
            className="w-full px-2 pb-0 pt-0"
            classNames={CALENDAR_CLASSNAMES}
            components={CALENDAR_COMPONENTS}
          />
        ) : view === "month" ? (
          <MonthGrid
            month={month}
            value={value}
            locale={locale}
            onSelect={handleMonthSelect}
          />
        ) : (
          <YearGrid
            yearPageStart={yearPageStart}
            value={value}
            onSelect={handleYearSelect}
          />
        )}
      </div>
    </div>
  )
}