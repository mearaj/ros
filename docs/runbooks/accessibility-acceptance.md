# Accessibility acceptance checklist

Record evidence for each item before checking the Section 15 UX gates.

## Critical Community flows

1. Unlock with Owner/Manager/Cashier/Kitchen PIN using keyboard only.
2. POS: search, add line, apply discount (manager), split tender, complete sale
   with mouse, touch (where available), and keyboard.
3. KDS: progress a ticket through `new → preparing → ready → completed` with
   keyboard focus visible.
4. Reports: open invoice detail, refund/void dialogs (including approver PIN),
   and day close without pointer-only controls.
5. Contrast: critical text and primary actions pass a WCAG AA smoke check on
   the reference desktop theme.
6. Text scale: OS large-text / Flutter text scale 1.3 keeps POS totals readable.
7. Screen reader smoke (platform narrator/Orca): lock screen, sale confirmation,
   and offline storage-attention states announce meaningful labels.

## Not claimed yet

- Formal accessibility certification.
- Mobile TalkBack/VoiceOver (mobile remains fail-closed).
