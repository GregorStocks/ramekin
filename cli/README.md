# Ramekin CLI

Command-line client using an auto-generated Rust client from the server's OpenAPI spec.

## Usage

```bash
cargo run -p ramekin-cli -- --help
cargo run -p ramekin-cli -- seed --username t --password t ../data/dev/seed.paprikarecipes
```

## Client Regeneration

The generated client in `generated/` is checked into git. It regenerates automatically when server API changes. To force regeneration:

```bash
make clean-api && make test
```
