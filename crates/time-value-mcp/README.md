# time-value-mcp

A [Model Context Protocol](https://modelcontextprotocol.io) server exposing
[`time_value`](../time_value)'s calculations as tools over stdio. Installs the
`time-value-mcp` binary.

Not published, and `publish = false` until the surface stops moving. From a
checkout:

```sh
cargo install --path crates/time-value-mcp   # installs `time-value-mcp`
time-value-mcp                               # speaks MCP over stdin/stdout
```

Point an MCP client at the binary; it will `initialize`, list the tools, and
call them.

## Tools

| tool                         | result                                      |
| ---------------------------- | ------------------------------------------- |
| `simple_future_value`        | `FV = PV(1 + rt)`, as `{"future_value": …}` |
| `simple_accumulation_factor` | `1 + rt`, as `{"accumulation_factor": …}`   |

Both are pure functions and say so: `readOnlyHint`, `idempotentHint`, and
`openWorldHint: false`.

## A rate is never a bare number

```json
{ "amount": 100, "rate": { "fraction": 0.05 }, "periods": 3 }
{ "amount": 100, "rate": { "percent": 5 },    "periods": 3 }
```

Those are the same rate. There is no default and no inference from magnitude,
and the schema expresses the choice as a `oneOf` rather than describing it in
prose. A bare `"rate": 5` is refused, as is naming both spellings at once.

`periods` counts the same period the rate is quoted per. The formula never asks
what that period is, so naming one is the caller's job.

Unknown fields are refused rather than dropped — a misspelled argument is how a
confident wrong answer happens.

## A failure names what would fix it

```json
{
  "code": -32602,
  "message": "accumulation factor `1 + rt` is `-0.5`, not positive",
  "data": { "kind": "domain" }
}
```

`domain` means the inputs were jointly meaningless — change the model.
`representation` means the arithmetic left what a 64-bit float holds — rescale.
The class comes from the library's `Error::kind`, so this server reports the
distinction rather than deciding it, and it is in `data` so a caller can branch
on it instead of matching prose.

A malformed _argument_ is a different thing from a refused _value_, and the two
arrive differently: the first is an error-flagged tool result, the second a
protocol error. Both carry a message that says which.

## Results are not rounded

`{"future_value": 114.99999999999999}` for 100 at 5% over 3 periods. That is the
answer; the library declines to invent precision and so does this.

**If you read that with Rust's `serde_json`, enable its `float_roundtrip`
feature.** Measured 2026-08-08: without it, `from_str` reads that number back as
`115.0` — serialisation is exact, deserialisation is not.

## Result keys mirror the tool name

`simple_future_value` returns `future_value`; the CLI's `simple fv` returns
`fv`. One name per operation _within_ a surface, and each surface names
operations for its own audience — a shell wants a short word, an agent wants a
legible one.

## License

Dual-licensed under [Apache-2.0](../../LICENSE-APACHE) or
[MIT](../../LICENSE-MIT), at your option.
