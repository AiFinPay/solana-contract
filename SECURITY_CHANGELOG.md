# Security changelog

## Unreleased

### High — checked oracle normalization

Pyth's exponent adjustment used unchecked addition, negation, and `u128::pow`.
An extreme feed exponent could panic the program instead of returning a
defined protocol error. The conversion now uses checked integer operations
throughout and has regression tests for normal negative/positive exponents,
fractional SOL, non-positive prices, and overflow boundaries.

### High — accounting panics and silent saturation

All reachable `unwrap()` calls in balance/accounting updates and all
`saturating_sub()` fee calculations were replaced with `MathOverflow` errors.
Failures now revert with a protocol error instead of panicking or silently
changing the credited amount.
