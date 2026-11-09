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
After building, open `dist/index.html` directly to preview from disk. The page
uses relative local assets so the same output also works from a GitHub Pages
project-site subpath; publish the contents of `dist/` as the Pages artifact.
