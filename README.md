# spill_ring

A `no_std` ring buffer that spills overflow to a configurable sink.

## Features

- `no-atomics` - Use `Cell` instead of atomics for single-context embedded systems

## License

MIT OR Apache-2.0
