# svg-renderer

A small HTTP service that renders a five-color palette preview as a PNG. It
fills the placeholders in [`colors.svg`](colors.svg) with the requested colors,
picks black or white label text for each swatch based on its brightness, and
rasterizes the result with [resvg](https://github.com/linebender/resvg).

## API

```
GET /render/<color1>/<color2>/<color3>/<color4>/<color5>
```

Each color is a six-digit hex value without the leading `#` (case-insensitive).

```sh
curl -o palette.png http://127.0.0.1:9278/render/1d1f21/c5c8c6/cc6666/b5bd68/81a2be
```

| Status | Body                                                                        |
| ------ | --------------------------------------------------------------------------- |
| `200`  | `image/png`, sent with `Cache-Control: public, max-age=31536000, immutable` |
| `400`  | Plain-text message naming the invalid color                                 |
| `500`  | Rendering failed; the cause is logged                                       |

## Running

```sh
cargo run --release
```

The server listens on `127.0.0.1:9278` by default. It uses Rocket's standard
configuration, so any setting can be overridden with a `Rocket.toml` file or
`ROCKET_*` environment variables:

```sh
ROCKET_PORT=8080 ROCKET_ADDRESS=0.0.0.0 cargo run --release
```

The SVG template is embedded at compile time, so the binary has no runtime file
dependencies. To run it under [pm2](https://pm2.keymetrics.io/), build a release
binary and use the included [`ecosystem.config.js`](ecosystem.config.js):

```sh
cargo build --release
pm2 start ecosystem.config.js
```

## Customizing the template

[`colors.svg`](colors.svg) may contain these placeholders, where `N` is `0`–`4`:

- `{bg_N}`: the Nth requested color
- `{text_N}`: `000000` or `FFFFFF`, whichever contrasts better with the Nth color

Both are substituted without the `#`, so write them as `fill="#{bg_0}"`.
Rebuild after editing the template.

## Development

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## License

[AGPL-3.0](LICENSE)
