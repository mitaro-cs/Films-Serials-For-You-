import { defineConfig, devices } from '@playwright/test';

// Сквозные проверки против собранного сервера (`cargo build --release -p fsfy-server`) на
// демо-данных из e2e/setup.ts: телефон, планшет и компьютер.
export default defineConfig({
  testDir: 'e2e',
  globalSetup: './e2e/setup.ts',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 30_000,
  expect: { timeout: 7_000 },
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    storageState: 'e2e/.state.json',
    locale: 'ru-RU',
    timezoneId: 'Europe/Moscow',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    { name: 'phone', use: { ...devices['iPhone 13'], browserName: 'chromium' } },
    { name: 'tablet', use: { viewport: { width: 820, height: 1180 }, hasTouch: true } },
    { name: 'desktop', use: { viewport: { width: 1440, height: 900 } } },
  ],
});
