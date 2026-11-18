# Herdr mod set-up

Follow these steps once on a new machine.

1. Run `mise install`. It installs Bun and, on Linux, `herdr-app` 0.1.16.
2. On macOS, install the Herdr app with `curl -fsSL https://releases.herdr.dev/app/install.sh | sh`.
   Herdr's macOS release needs an Apple Silicon Mac; the installer refuses Intel Macs, which must
   build Herdr from source. The installer puts `herdr-app` in `~/.local/bin`. Add `~/.local/bin` to
   `PATH` before `pnpm install`, then check that `command -v herdr-app` prints a path.
3. Run `pnpm install`. It runs `herdr-app types mods/{{name}}`, which writes `mods/{{name}}/gpui-kit.d.ts`.
   Git ignores that file. Run `pnpm install` again after a Herdr upgrade.
4. Run `pnpm run build`. It bundles the mod into `mods/{{name}}/main.js`.
5. Run `herdr-app check mods/{{name}}`. It loads the bundle and exits 0 when Herdr accepts the mod.
6. Run `pnpm run check`. It runs `check:layer:herdr`, which builds the mod and runs
   `herdr-app check mods/{{name}}`.
7. Run `pnpm run mod:install`. It builds the mod and copies `main.js` and `mod.json` to
   `$HERDR_APP_HOME/mods/{{name}}` when that is set, else `$XDG_CONFIG_HOME/herdr-app/mods/{{name}}` when
   that is set, else `~/.config/herdr-app/mods/{{name}}`. The first time Herdr sees the folder, it docks
   the mod on the right. Run it again after each change; Herdr reloads the mod.
