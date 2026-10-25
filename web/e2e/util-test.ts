import { test as base, expect } from '@playwright/test';

// Любая ошибка JavaScript на странице - провал теста (а не «страница просто пустая»).
export const test = base.extend<{ errors: string[] }>({
  errors: [
    async ({ page }, use) => {
      const errors: string[] = [];
      page.on('pageerror', (e) => errors.push(e.message));
      await use(errors);
      expect(errors, 'ошибки на странице').toEqual([]);
    },
    { auto: true },
  ],
});
export { expect };
