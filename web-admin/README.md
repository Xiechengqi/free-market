# free-market web-admin

The Vue3 + NaiveUI + UnoCSS single-page admin shell for **free-market**.

This directory builds to `web-admin/dist/`, which the Rust binary embeds via
`rust-embed` and serves under `/admin/`. There is no separate frontend
service — `cargo build --release` runs `pnpm -C web-admin build` first (see
`/build.sh`) and the resulting `dist/` becomes part of the binary.

## Stack

- Vue 3, Vite, TypeScript
- naive-ui, UnoCSS
- Pinia, vue-router, vue-i18n
- elegant-router (auto-generated routes from `src/views/`)

Originally forked from
[SoybeanAdmin](https://github.com/soybeanjs/soybean-admin) v2.2.0. Most of the
layout, theme drawer, and tooling code is unchanged; the business pages,
brand wiring (`useSiteInfoStore`), setup wizard, and auth flow are freeMarket
specific.

## Development

```sh
pnpm install
pnpm dev          # vite, talks to a local free-market on http://localhost:8080
pnpm build        # produces dist/ for the Rust binary to embed
pnpm typecheck
pnpm lint
```

The admin SPA expects the backend at `/admin/api/*` (same origin in prod).
Brand info (`name`, `logoText`, `imgLogo`, `footer`, `language`) is fetched
from `/admin/api/site-info` on boot and centralized in `useSiteInfoStore`;
operators change those values under `/admin/settings`.

## License

MIT — see [LICENSE](./LICENSE).
