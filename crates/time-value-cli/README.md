# time-value-cli

The command-line interface for [`time_value`](../time_value). Installs the
`time-value` binary.

Not published, and the crate is `publish = false` until its surface stops
moving. From a checkout:

```sh
cargo install --path crates/time-value-cli   # installs `time-value`
```

## Usage

Commands are noun-then-verb, where the noun is the interest model. A rate is
always given explicitly as a fraction or a percentage — the two conflict, one is
required, and neither is inferred from magnitude. `--periods` counts the same
period the rate is quoted per; the formula never asks what that period is.

```sh
time-value simple fv     --amount 100 --rate 0.05 --periods 3
time-value simple fv     --amount 100 --rate-percent 5 --periods 3
time-value simple factor --rate 0.05 --periods 3

time-value --json simple fv --amount 100 --rate 0.05 --periods 3
```

Negative values are ordinary: a negative amount is a liability that accumulates
into a larger one, and a negative rate shrinks an amount.

```sh
time-value simple fv --amount -100 --rate -0.05 --periods 3   # -85
```

## The number is printed in full

```sh
$ time-value simple fv --amount 100 --rate 0.05 --periods 3
114.99999999999999
```

That is the answer, not a display artifact: `100` times a factor of `1.15` is
not `115` in binary floating point. Rounding it here would be the CLI inventing
a precision the library declines to — it has a `Tolerance` type and an
`is_close` because the gap is real. Printed this way the output also
round-trips, being exactly what `Amount`'s `FromStr` reads back.

## Exit codes

A failed run's code names **what would fix it**, not what went wrong.

| code | meaning                                                              | remedy                           |
| ---- | -------------------------------------------------------------------- | -------------------------------- |
| `0`  | an answer, on stdout                                                 | —                                |
| `1`  | a **domain** failure: the inputs were jointly meaningless            | change the model                 |
| `2`  | the arguments did not parse, from `clap` before anything is computed | fix the command line             |
| `3`  | a **representation** failure: the arithmetic left what a float holds | rescale, or carry more precision |

Those two remedies are opposite, which is why they are different codes:

```sh
$ time-value simple factor --rate -0.5 --periods 3
error (domain): accumulation factor `1 + rt` is `-0.5`, not positive
$ echo $?
1

$ time-value simple fv --amount 1.7976931348623157e308 --rate 1 --periods 1
error (representation): product is not finite
$ echo $?
3
```

The class comes from the library's `Error::kind`, so this crate reports the
distinction rather than deciding it.

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) or
[MIT](../../LICENSE-MIT), at your option.
