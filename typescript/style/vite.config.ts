import { defineConfig } from 'vite-plus';

import { format, lint, vitest } from './src/index.ts';

export default defineConfig({
  lint: {
    extends: [lint, vitest],
    ignorePatterns: ['dist/**'],
  },
  fmt: {
    ...format,
    // Local agent state is not source and must not be reformatted.
    ignorePatterns: ['pnpm-lock.yaml', '.tau/**', 'dist/**'],
  },
  test: {
    include: ['src/**/*.test.ts', 'tests/**/*.test.ts'],
    globalSetup: ['tests/packTarball.ts'],
  },
});
