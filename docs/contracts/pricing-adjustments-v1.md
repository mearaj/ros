# Pricing adjustments contract v1

Status: storage and Community UI integration are implemented for provider-neutral
integer tax treatments and owner/manager order discounts (fixed and percentage).
POS split tender uses a trusted payable preview before allocations. Branch tax
rates and product tax treatments are editable in Menu. This is still not an
assertion of Indian GST compliance, e-invoicing, HSN/SAC classification, or any
other jurisdiction-specific fiscal claim.

## Purpose and boundary

`ros_core::pricing` defines deterministic, provider-neutral arithmetic for restaurant line taxes and controlled order discounts. It produces an immutable Rust breakdown that can later be copied into an append-only invoice snapshot.

This contract is not an assertion of Indian GST compliance, or of compliance with any other jurisdiction. Tax registration identifiers, HSN/SAC classification, place-of-supply rules, fiscal invoice numbering or formatting, returns and filings, e-invoicing, exemptions, jurisdiction policy, and tax-provider integration remain explicitly deferred.

## Implemented local boundary

- Branch tax rates are archive-only named basis-point components (at most eight active).
- Products carry `no_tax`, `exclusive`, or `inclusive` treatment; exclusive/inclusive sales apply every currently active branch rate.
- `complete_sale` calls `ros_core::pricing::calculate_pricing`, persists net/discount/tax/payable totals plus a pricing snapshot, and requires manager/owner authority when a discount is present.
- Community POS calls a non-recording pricing preview so split tender and checkout confirmations use the same payable as Rust will persist.
- Receipt reprint and POS checkout expose discount/tax lines when non-zero.
- Menu lists/creates/archives branch tax rates and sets each product's `no_tax` / `exclusive` / `inclusive` treatment.

## Numeric model

- Every price, tax, discount, cap, subtotal, and payable is an integer `i64` count of currency minor units. Floating-point arithmetic is forbidden.
- All lines in one calculation use the same strict three-letter uppercase currency code.
- Unit prices, taxes, discount values, and results are nonnegative.
- Quantity is a positive whole number no greater than 1,000,000.
- All multiplication uses a wider integer intermediate and every public result, sum, and payable is checked for `i64` overflow. Overflow fails the whole calculation.
- A successful payable can never be negative.

## Tax input

A line has exactly one treatment:

- `NoTax`: the supplied price is net and has no tax components.
- `Exclusive`: the supplied price is net; calculated components are added to it.
- `Inclusive`: the supplied price is gross; the aggregate included tax is back-calculated and the gross is preserved exactly.

A taxable profile contains one through eight normalized, uniquely named components. Each component and their combined rate must be between 0 and 10,000 basis points inclusive. A zero-rate taxable profile remains distinct from `NoTax` so a future invoice can preserve the operator's stated treatment. The 100% ceiling is a defensive v1 product bound, not a statement of any jurisdiction's permitted rates.

Tax component names are trimmed, NFC-normalized, control-character-free, and limited to 120 Unicode characters. Components are stored in normalized-name order so equal-remainder allocation is stable regardless of the order supplied by a UI.

## Rounding and inclusive back-calculation

The only v1 rounding rule is nearest minor unit, with exact halves rounded up. All operands are nonnegative. This is a common commercial rule, is reproducible across Rust and future clients, and avoids silently choosing the lower charge at midpoint ties.

For a net amount `N`, component rate `R`, and basis-point denominator `B = 10,000`, exclusive tax is:

```text
round_half_up(N × R / B)
```

For an inclusive gross `G` and combined rate `T`, aggregate included tax is:

```text
round_half_up(G × T / (B + T))
```

Net is exactly `G - included_tax`. The aggregate included tax is apportioned to named components with the largest-remainder method, weighted by component basis points. Remainder ties use normalized tax name order. Consequently, component taxes always add back to the exact aggregate and `net + tax == supplied gross` before a discount.

## Order discounts

An order has either no discount or one validated order discount:

- `Fixed`: a nonnegative minor-unit amount.
- `Percentage`: 0 through 10,000 basis points, with an optional nonnegative maximum minor-unit cap.

Every discount requires a trimmed, NFC-normalized, control-character-free reason of at most 500 Unicode characters. Both its display form and normalized comparison key are retained in the calculation breakdown.

A percentage is calculated on the order's pre-discount net subtotal using the v1 rounding rule, then reduced by its explicit cap when present. A fixed or percentage result is finally capped at the net subtotal. This subtotal cap guarantees that discount allocation and payable totals cannot become negative.

The applied discount is allocated across lines in proportion to each line's pre-discount net amount. Integer remainders use the largest-remainder method; ties use normalized line-reference order. Allocations therefore add exactly to the applied order discount and do not depend on UI line ordering.

After allocation:

- no-tax lines continue to have zero tax;
- exclusive component tax is recalculated from discounted line net using its stated rate;
- inclusive component tax is reduced in proportion to remaining line net using the v1 rounding rule, preserving the original inclusive component decomposition as the rounding anchor.

At a sub-minor-unit edge, inclusive back-calculation can classify a positive one-minor-unit gross entirely as rounded tax and leave a zero net base. No net discount can be allocated to that line, so its included tax is preserved.

## Bounds and validation

- An order contains 1 through 200 pricing lines.
- Line references and display names are trimmed, NFC-normalized, bounded, and control-character-free.
- Line references are unique by normalized case-insensitive comparison key.
- Negative unit prices, empty taxable profiles, duplicate tax names, excessive rates, excessive collections, mixed currencies, and arithmetic overflow fail closed.

These bounds protect checkout and future snapshot payloads from pathological inputs. They are product safety limits, not fiscal policy.

## Immutable output and invariants

All breakdown fields are private and exposed through read-only accessors. A successful result retains:

- currency and the named rounding rule;
- normalized line identity and display facts;
- supplied unit price, quantity, treatment, and extended input price;
- net, allocated discount, tax, and payable before and after discount;
- every tax component's normalized name, basis points, and amount before and after discount;
- discount method parameters, explicit cap, calculated amount, applied amount, reason, and authorization intent;
- complete order-level input, net, discount, tax, and payable totals.

The calculation guarantees:

```text
sum(line allocated discount) == order discount
pre-discount net - discount == post-discount net
sum(line net) == order net
sum(line tax) == order tax
line net + line tax == line payable
order net + order tax == order payable
sum(line payable) == order payable
order payable >= 0
```

## Authorization intent

Every discount carries `ManagerOrOwnerApproval` intent. Owner and manager are the intended approving roles; cashier and kitchen roles are forbidden. This metadata is not authorization evidence. Later storage integration must revalidate the authenticated actor, current role, active session, branch, and device atomically in the same transaction that appends the discount and invoice facts. Flutter visibility or a caller-supplied role must never authorize a discount.

## Verification

The module tests cover zero values and rates, midpoint rounding, inclusive back-calculation, component apportionment, fixed and percentage caps, mixed treatments, normalization and collection bounds, role intent, currency mismatch, multiplication/summation/payable overflow, and a small-domain invariant sweep.

Run the focused verification with:

```bash
cargo fmt -p ros_core -- --check
cargo check --locked -p ros_core
cargo test --locked -p ros_core
cargo clippy --locked -p ros_core --all-targets -- -D warnings
```
