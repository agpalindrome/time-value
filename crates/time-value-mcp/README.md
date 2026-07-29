# time-value-mcp

A [Model Context Protocol](https://modelcontextprotocol.io) server exposing
[`time_value`](../time_value)'s time-value-of-money calculations as tools over
stdio. Installs the `time-value-mcp` binary. Its design is
[ADR-0011](../../docs/adr/0011-mcp-server.md); async is contained to this crate,
the core library stays synchronous.

## Tools

| Tool | Result |
|------|--------|
| `npv`, `nfv` | net present / future value of a cashflow series at a per-period rate |
| `irr` | internal rate of return of a series |
| `mirr` | modified internal rate of return (finance + reinvestment rates) |
| `xnpv`, `xirr` | net present value / internal rate of return of cashflows on irregular ISO dates, at an annual rate |
| `single_sum_present_value`, `single_sum_future_value` | a single sum over a number of periods |
| `single_sum_periods`, `single_sum_rate` | solve a single sum for periods (NPER) or rate (RATE) |
| `annuity_present_value`, `annuity_future_value`, `annuity_payment` | ordinary (end-of-period) annuities |
| `annuity_periods`, `annuity_rate` | solve an annuity for periods / rate, from a present or future value |
| `annuity_perpetuity`, `annuity_growing_perpetuity` | present value of a (growing) perpetuity |
| `annuity_growing_present_value`, `annuity_growing_future_value` | finite growing annuity — a payment that grows each period (the rate need not exceed the growth) |
| `annuity_due_present_value`, `annuity_due_future_value`, `annuity_due_payment` | annuity-due (start-of-period) |
| `annuity_growing_due_present_value`, `annuity_growing_due_future_value` | finite growing annuity-due |
| `rate_effective_annual`, `rate_convert`, `rate_from_nominal`, `rate_nominal` | rate conversions (each takes a periodicity) |
| `amortize` | an amortization schedule (array of rows) from a term or a level payment |

Rates are per period (annual for `xnpv`/`xirr`); cashflows are signed (outflow
negative). `xnpv`/`xirr` take `{date, amount}` flows with ISO `YYYY-MM-DD` dates,
discounted by year-fraction (ACT/365) from the first date. Each tool returns a
one-field structured JSON result keyed by the operation.

## Install

Not yet published to crates.io (see [ADR-0012](../../docs/adr/0012-ci-and-release-automation.md)).
From a checkout of the repository:

```sh
cargo install --path crates/time-value-mcp   # installs the `time-value-mcp` binary
```

## Running

```sh
time-value-mcp   # speaks MCP JSON-RPC over stdin/stdout
```

Point an MCP client (e.g. an assistant that speaks MCP) at the binary; it will
`initialize`, list the tools, and call them.

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) or [MIT](../../LICENSE-MIT),
at your option.
