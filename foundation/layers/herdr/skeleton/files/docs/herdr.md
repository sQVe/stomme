# Herdr mod set-up

Complete the [development guide setup](development.md#set-up) through `mise install` first. Then
follow these steps once on a new machine.

1. Run `mise install`. It installs Bun and, on Linux, `herdr-app` 0.1.16.
2. On macOS, install the Herdr app with `curl -fsSL https://releases.herdr.dev/app/install.sh | sh`.
3. Run `pnpm install`. It runs `herdr-app types mods/{{name}}`, which writes `mods/{{name}}/gpui-kit.d.ts`.
   Git ignores that file. Run `pnpm install` again after a Herdr upgrade.
4. Run `pnpm run build`. It bundles the mod into `mods/{{name}}/main.js`.
5. Run `herdr-app check mods/{{name}}`. It loads the bundle and exits 0 when Herdr accepts the mod.
6. Run `pnpm run check`. It runs `check:layer:herdr`, which builds the mod and runs
   `herdr-app check mods/{{name}}`.
