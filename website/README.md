# H5Jive project page

This is a static Astro site introducing the H5Jive desktop application.
It is intentionally independent of the Rust app and needs no database or
runtime service.

From the repository root, start the local site with `./h5jive.sh --website`.
Astro prints the preview URL (normally `http://127.0.0.1:4321/`); press Ctrl+C
to stop the server. Install dependencies first if needed:

```sh
cd website && npm install
```

```sh
npm install
npm run dev
npm run build
npm run preview
```

Astro writes the static site to `dist/`. The project guide lives in `../docs/`.
The local Astro server serves only the website, so `/guide/` links are available
after the Pages workflow builds and copies the mdBook guide, not during a plain
local website preview.
The [Pages workflow](../.github/workflows/pages.yml) builds both and publishes
the guide at `/guide/`. It runs when the root README, documentation, or website
changes on `main`.

## Keeping screenshots in sync

Application screenshots live in the repository-root `assets/` directory, while
the website serves its copies from `website/public/`. After regenerating or
replacing a screenshot, copy the fresh files into the public directory with:

```sh
./scripts/sync-website-images.sh
```

The script only writes PNG files that are missing or different in
`website/public/`, leaving website-only files such as the logo and favicon
untouched.
