# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Finance and tax staff at one company group who run the monthly VAT and stamp-duty cycle. They work at a Windows desktop, in Simplified Chinese, and spend long sessions in dense tables. An administrator role exists and is the only one who can unlock locked tax periods.

## Product Purpose

A Tauri desktop app that imports invoices and vouchers from Excel, then produces the input-tax transfer-out (进项转出) and stamp-duty (印花税) results for each tax period, ready to file. Success means the filing list matches history exactly, because back-tax audits recompute against the original tax period.

## Positioning

Period-run workflow with a batch lifecycle (pending confirmation → confirmed → locked). Locking is permanent, and later changes are recorded as supplementary batches instead of edits.

## Operating Context

- Imports from Excel: invoices (including railway and passenger tickets), certified lists, AP / IC / bank vouchers, travel expenses, and tax rules.
- A tax period run (纳税所属期) is a single month or up to 3 consecutive months, in YYYYMM.
- Outputs include the stamp-duty collection spreadsheet, named `{纳税所属期}_印花税税源信息采集表.xlsx`.
- Other features: invoice and reversal queries, input-tax transfer review and history, rule management, admin.

## Capabilities and Constraints

- Batch states: pending confirmation → confirmed (read-only, exportable, can be unconfirmed) → locked (requires accounting voucher number, posting date, and preparer; permanently read-only).
- Only an admin can unlock. Unlocking opens a supplementary batch n+1 and does not undo earlier results.
- Confirmed or locked line items are excluded from matching in other periods.
- Voucher column naming must distinguish the "original posting voucher" (AP / IC / bank / travel) from the "transfer-out / stamp-duty voucher".
- The tax-period pages switch periods in place and offer "confirmed only" and "all" views. There is no separate query page.
- Development stage: no data migration. Change the create-table statements directly.

## Evidence on Hand

Real code and flows already exist under `src/features` and `src-tauri/src/features`. No external user research or testimonials are on record.

## Product Principles

1. Accuracy over speed: amounts, periods, and sources must be unambiguous, and import errors must be traceable to a cell or row.
2. State must be legible: confirmed, locked, and supplementary status is always visible, and read-only data never looks editable.
3. History is immutable once locked: corrections happen through supplementary batches.
4. Dense and scannable: tables and rows for repeated monthly work.
5. Chinese-first terminology, used consistently.

## Brand Commitments

Reuse the existing shadcn component wrappers (including the wrapped input and combobox) for tax-period pages.

## Open Decisions

- Resolved 2026-11-09: shadcn (base-nova) wrappers plus Tailwind v4 tokens are authoritative. The Astryx rules in `.claude/CLAUDE.md` are stale: Astryx is not installed and nothing imports it.
