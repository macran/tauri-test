---
name: Tauri Tax Desk
description: Dense, calm desktop tooling for monthly VAT transfer-out and stamp-duty filing.
colors:
  ink-blue: "#2d4fad"
  ink-blue-night: "oklch(0.620 0.175 254)"
  pearl-canvas: "#f2f4f8"
  night-canvas: "#0d1829"
  deep-ink: "#1a2a42"
  harbor-sidebar: "oklch(0.250 0.075 252)"
  field-border: "oklch(0.800 0.020 250)"
  success-green: "#0f9e72"
  caution-amber: "#c08000"
  alert-red: "#d92b2b"
  violet-note: "#6d3bb8"
  teal-note: "#0f766e"
typography:
  body:
    fontFamily: "HarmonyOS Sans, Microsoft YaHei UI, Segoe UI, Noto Sans SC Variable, system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 400
  button:
    fontFamily: "HarmonyOS Sans, Microsoft YaHei UI, Segoe UI, Noto Sans SC Variable, system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 500
  label:
    fontFamily: "HarmonyOS Sans, Microsoft YaHei UI, Segoe UI, Noto Sans SC Variable, system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 400
  caption:
    fontFamily: "HarmonyOS Sans, Microsoft YaHei UI, Segoe UI, Noto Sans SC Variable, system-ui, sans-serif"
    fontSize: "0.6875rem"
    fontWeight: 400
  mono:
    fontFamily: "Geist Variable, ui-monospace, JetBrains Mono, monospace"
    fontSize: "0.8125rem"
rounded:
  sm: "2.9px"
  md: "3.9px"
  lg: "5.2px"
  xl: "7.3px"
components:
  button-primary:
    backgroundColor: "{colors.ink-blue}"
    textColor: "#ffffff"
    rounded: "{rounded.lg}"
    height: "28px"
    padding: "0 10px"
  button-sm:
    height: "28px"
    rounded: "{rounded.md}"
  field:
    backgroundColor: "#ffffff"
    textColor: "{colors.deep-ink}"
    rounded: "{rounded.lg}"
    height: "28px"
---

# Design System: Tauri Tax Desk

## Overview

**Creative North Star: "The Ledger Desk"**

A calm, exact accountant's desk: cool paper, one ink-blue accent, and everything legible and countable. The interface serves long monthly sessions with Chinese text, amounts, and period states, so scanability and consistency come before expression.

Two modes share one structure. Light ("Warm Pearl") is a pale blue-grey canvas with near-white translucent cards. Dark ("Deep Indigo") is a deep navy canvas with faint white-alpha surfaces. The sidebar is a dark navy panel in both modes.

**Key Characteristics:**
- One accent (ink blue) on cool blue-grey neutrals; semantic hues only for status.
- Compact controls: every field and default button is one height (`--control-h`, 28px). Edge-to-edge table rows.
- Small radius (5.2px base), light glass layering, soft blue-tinted shadows.
- Chinese-first type stack.

## Colors

A cool blue-indigo family with a single accent; green, amber, red, violet, and teal appear only as status or category cues.

### Primary
- **Ink Blue** (#2d4fad light; oklch(0.620 0.175 254) dark): primary buttons, active nav, focus ring, selected and accent states. Menu and dropdown highlights use the same blue.

### Neutral
- **Pearl Canvas** (#f2f4f8): light app background; **Night Canvas** (#0d1829): dark app background.
- **Deep Ink** (#1a2a42): light-mode foreground text. Muted text steps through #22344c, #55688a, #7c93b3.
- **Harbor Sidebar** (oklch(0.250 0.075 252)): sidebar surface in light mode, darker in dark mode.
- **Field Border** (oklch(0.800 0.020 250)): default input border; deepens on hover, turns primary on focus.

### Status
- **Success Green** (#0f9e72), **Caution Amber** (#c08000), **Alert Red** (#d92b2b), **Violet Note** (#6d3bb8), **Teal Note** (#0f766e), each with a 9-10% alpha background companion.

### Named Rules
**The One Voice Rule.** Ink blue is the only accent. Status colors mark state, never decoration.

## Typography

**Body Font:** HarmonyOS Sans → Microsoft YaHei UI → Segoe UI → Noto Sans SC Variable
**Mono Font:** Geist Variable (digits and codes)

**Character:** Neutral system sans tuned for Simplified Chinese on Windows.

### Hierarchy
- **Body** (400, 14px): table cells, form values, descriptions.
- **Button** (500, 14px; 12.8px at `sm`, 12px at `xs`): controls.
- **Label** (400, 12px): field labels above controls, table headers, helper text, inline notes.
- **Caption** (400, 11px): title-bar and dashboard secondary text only; not for form content.
- **Mono** (13px): voucher numbers, amounts, periods where alignment matters.

No display scale exists; headings reuse the sans stack through `--font-heading`.

## Layout

Desktop-only window with a custom title bar, a left sidebar, and tabbed pages with sub-tabs. Content is dense: tables run edge-to-edge in a card, controls sit in compact toolbars. Spacing follows Tailwind's 4px scale. There is no mobile breakpoint work. The main window has a hard minimum of 1100×700 (`tauri.conf.json`), so layouts are designed for 1100px and up; forms still wrap cleanly between that and a maximized window.

**Form structure.** A form is one flow of fields followed by a separate command group. Everything that scopes the query, including 布局 (table layout), is a field with its label above the control. Commands (高级查询 link, 导出, 重置, 查询/匹配) sit together at the right and wrap as a group. The tax-period forms use a grid of 9-12rem columns with a reserved one-line error row; the query pages' `FieldShell` uses fixed 150px (300px for date ranges) fields in a wrapping flow, which is intentional: a field that does not fit moves to the next row whole instead of being squeezed.

## Elevation & Depth

Hybrid: tonal translucent surfaces with a light backdrop blur (6-8px) and soft blue-tinted shadows. Depth is ambient, not dramatic.

### Shadow Vocabulary
- **Glass** (`0 2px 12px rgba(30,50,100,.08), 0 1px 3px rgba(30,50,100,.05)`): resting cards.
- **Glass hover** (`0 6px 24px rgba(30,50,100,.13), 0 2px 6px rgba(30,50,100,.08)`): hovered cards.
- **Glass large** (`0 12px 40px rgba(20,40,90,.14), ...`): dialogs and popovers.

Dark mode swaps these for black-based shadows of higher opacity.

### Motion
Motion confirms a state change and nothing else: button hover lift (1px) and press (scale 0.98) at 120-150ms, badge color ease at 200ms, dialogs scale-fade from 95% at 100ms, lock icon pop-in once. Every animation has a reduced-motion fallback that keeps the color change and drops movement.

### Named Rules
**The Light Glass Rule.** Blur stays at or below 10px; surfaces remain readable without it.

## Shapes

Small, precise corners: 5.2px base radius (`--radius` 0.325rem), scaled to 2.9 / 3.9 / 5.2 / 7.3px. Cards use the larger steps. Borders are 1px, low-contrast blue-grey; inputs have a visible neutral border.

## Components

### Buttons
- **Shape:** 5.2px radius, 28px tall (`--control-h`; `sm` is also 28px, `xs` 24px).
- **Primary / AppButton brand:** solid ink-blue fill (no gradient), white text; hover lifts 1px with a deeper blue shadow, press scales to 0.98. `glass` (secondary) and `danger` share the same motion. Motion drops to color-only under reduced motion.
- **Reset / secondary command:** `AppButton variant="glass"` with the `ArrowCounterClockwise` icon, always icon plus text. **Query / match / confirm:** `variant="brand"` with its icon. **Lock:** `variant="danger"`. Raw shadcn `Button` is for dialogs and one-off cases; command rows use `AppButton`.
- **Outline / Secondary / Ghost / Destructive / Link:** from the shadcn base-nova set; destructive is a tinted red, not solid.
- **Focus:** 3px ring at 50% ring color; active nudges 1px down.

### Inputs / Fields
- **Height:** every field control (input, select, combobox, date picker, toggle) uses `--control-h` (1.75rem), the same as the default button, so a row of mixed controls shares one baseline.
- **Style:** white (light) or 6% white (dark) fill, neutral border; wrapped `input`, `combobox`, and `option-combobox` are the standard controls.
- **Focus:** border turns primary with ring. **Error:** destructive border and ring.

### Cards / Containers
- **Corner:** large radius, 1px translucent border, glass shadow. `LiquidCard` is the project card wrapper.

### Switch, tabs, status
- **Binary filter:** a `Switch` with its label beside it (e.g. 仅已确认, default off = show all). Radio groups are for three-way or exclusive choices such as 查询对象.
- **Tab strip (reversal query):** the active tab is a solid ink-blue segment with white text; inactive tabs are muted text. No counts in tab labels.
- **Batch state badge:** 待确认 outline, 已确认 pale green fill with a green check, 已锁定 solid primary with a lock icon; color changes ease over 200ms.

### Tables
- Header on a pale blue tint (#e8eefc), row hover at 6% ink blue, hairline row borders.

### Navigation
- Dark navy sidebar; active item is a solid ink-blue pill with a blue glow. Pages use tabs and sub-tabs.

## Do's and Don'ts

### Do:
- **Do** build with the shadcn/base-ui wrappers in `src/components/ui` and the CSS variables in `src/index.css`.
- **Do** make confirmed and locked states visibly read-only, and keep pending, confirmed, and locked visually distinct.
- **Do** put amounts, periods, and voucher numbers in a consistent, aligned format.

### Don't:
- **Don't** add a second accent color or decorative gradients; background orbs are already at the limit.
- **Don't** hard-code colors; use the tokens (`--primary`, `--text-*`, `--accent-*`).
- **Don't** wrap dense data rows in individual cards.
- **Don't** follow the Astryx rules; that library is not installed or used.
- **Don't** hard-code `h-7`/`h-8` on a field or command; use `--control-h`.
- **Don't** put layout or other query-scoping choices in the command group; they are fields.
