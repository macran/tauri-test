# CLAUDE.md

Project-specific guidance for AI coding agents.

Tauri desktop app: React, Tailwind v4, shadcn base-nova on @base-ui. Chinese UI. Product context is in `PRODUCT.md`; the visual system is in `DESIGN.md`.

RULES:
- Build UI from `src/components/ui` (shadcn wrappers) and the project wrappers (`app-button`, `liquid-ui`, wrapped input / combobox / `Win11DatePicker`). Do not add another component library.
- Colors, radius and shadows come from the CSS variables in `src/index.css` (`--primary`, `--text-*`, `--accent-*`, `--glass-*`). No raw hex in components.
- Dense data = table rows edge-to-edge; do not wrap rows in cards.
- Status uses the `--accent-*` tokens plus an icon or text label, never color alone. Confirmed and locked batches must look read-only.
- Form labels use `htmlFor` + `id`; errors link via `aria-describedby`.
- Voucher columns: keep "原记账凭证" (AP/IC/bank/travel) distinct from "转出/印花税凭证".
- Development stage: change create-table statements directly, no data migration.
