# Restaurant Operating System — Website

Marketing and download site for **Restaurant Operating System** by GOTIGIN.

- **Stack:** Next.js, TypeScript, React, Tailwind CSS, shadcn/ui
- **Theme:** Gotigin brand tokens (same palette as [gotigin.com](https://gotigin.com))
- **Domain:** [ros.gotigin.com](https://ros.gotigin.com)
- **Hosting:** Firebase App Hosting

## Develop

```bash
cd ros-website
npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000).

## Downloads

Community Edition binaries and signatures go in `public/downloads/`.
See that folder’s README and `src/lib/downloads.ts`.

| Platform | Status |
|----------|--------|
| Linux | Available (when binary published) |
| Windows | Available (when binary published) |
| Android (phone & tablet) | Available (when binary published) |
| macOS | Planned |
| iOS | Planned |

## Editions

- **Community** — free, downloadable now
- **Professional** — coming soon (trial/paid handled in-product)
- **Enterprise** — paid, coming soon

## Deploy (Firebase App Hosting)

Configured via `apphosting.yaml`, `firebase.json`, and `.firebaserc` (project `gotigin-a2869`, shared with gotigin.com).

```bash
npm run build
# Then connect this repo/folder as an App Hosting backend and map ros.gotigin.com
# or use: firebase deploy (with App Hosting backend configured)
```

Point the custom domain **ros.gotigin.com** to the App Hosting backend in the Firebase console.
