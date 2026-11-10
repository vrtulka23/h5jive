# H5Jive project page

This is a static Astro site introducing the H5Jive desktop application.
It is intentionally independent of the Rust app and needs no database or
runtime service.

```sh
npm install
npm run dev
npm run build
npm run preview
```

Astro writes the static site to `dist/`. The project guide lives in `../docs/`.
The [Pages workflow](../.github/workflows/pages.yml) builds both and publishes
the guide at `/guide/`. It runs when the root README, documentation, or website
changes on `main`.
