import { request } from '@playwright/test';
import { writeFileSync } from 'node:fs';
import { expect, test } from './util-test';
import { env, stranger, url } from './util';

test('безопасность: границы сессии, права, CSRF, изоляция прогресса', async ({}, info) => {
  test.skip(info.project.name !== 'desktop', 'один сервер');
  const guest = await request.newContext({ baseURL: env().publicURL });
  for (const path of ['/api/library', '/api/admin/settings', '/api/me', '/api/links/1']) {
    expect((await guest.get(path)).status(), path).toBe(401);
  }
  const viewer = await stranger('Проверка безопасности');
  expect((await viewer.c.get('/api/admin/settings')).status()).toBe(403);
  const host = await request.newContext({ baseURL: env().baseURL, storageState: 'e2e/.state.json' });
  const csrf = await host.put('/api/admin/settings', { data: { name: 'не должно сохраниться' } });
  expect(csrf.status()).toBe(403);
  const link = await host.post('/api/links', { headers: { 'x-fsfy': '1' }, data: { url: 'https://example.com/security.mp4' } });
  expect(link.ok()).toBe(true);
  const { id } = await link.json();
  expect((await viewer.c.put(`/api/links/${id}/progress`, { data: { position: -1, duration: 100 } })).status()).toBe(400);
  expect((await viewer.c.put(`/api/links/${id}/progress`, { data: { position: 23, duration: 100 } })).ok()).toBe(true);
  expect((await (await host.get(`/api/links/${id}`)).json()).progress).toBeNull();
  const page = await guest.get('/');
  expect(page.headers()['x-content-type-options']).toBe('nosniff');
  expect(page.headers()['content-security-policy']).toBeTruthy();
  expect((await guest.get('/img/..%2F..%2FCargo.toml')).status()).not.toBe(200);
  const status = await (await host.get('/api/admin/status')).json();
  if (process.platform === 'win32') {
    expect(status.system.supported).toBe(true);
    expect(status.system.memoryTotal).toBeGreaterThan(0);
    await new Promise((r) => setTimeout(r, 1100));
    const next = await (await host.get('/api/admin/status')).json();
    expect(next.system.cpu).toBeGreaterThanOrEqual(0);
    expect(next.system.cpu).toBeLessThanOrEqual(100);
  }
  await Promise.all([guest.dispose(), viewer.c.dispose(), host.dispose()]);
});

test('нагрузка: 32 клиента, 640 запросов каталога и прогресса', async ({ request: api }, info) => {
  test.skip(info.project.name !== 'desktop', 'один сервер');
  test.setTimeout(120_000);
  const created = await api.post(url('/api/links'), { headers: { 'x-fsfy': '1' }, data: { url: 'https://example.com/load.mp4' } });
  expect(created.ok()).toBe(true);
  const { id } = await created.json();
  const latencies: number[] = [];
  const statuses: number[] = [];
  const start = performance.now();
  await Promise.all(Array.from({ length: 32 }, async (_, worker) => {
    for (let i = 0; i < 20; i++) {
      const at = performance.now();
      const res = i % 4 === 0
        ? await api.put(url(`/api/links/${id}/progress`), { headers: { 'x-fsfy': '1' }, data: { position: worker + i, duration: 3600 } })
        : await api.get(url(i % 2 ? '/api/library' : '/api/health'));
      statuses.push(res.status());
      latencies.push(performance.now() - at);
      await res.dispose();
    }
  }));
  latencies.sort((a, b) => a - b);
  const result = {
    concurrency: 32, requests: statuses.length, failures: statuses.filter((s) => s !== 200).length,
    elapsedMs: Math.round(performance.now() - start),
    p50Ms: Math.round(latencies[Math.floor(latencies.length * .5)]),
    p95Ms: Math.round(latencies[Math.floor(latencies.length * .95)]),
    maxMs: Math.round(latencies.at(-1)!),
  };
  writeFileSync('test-results/load.json', JSON.stringify(result, null, 2));
  console.log('LOAD', JSON.stringify(result));
  expect(result.failures).toBe(0);
  const title = await (await api.get(url(`/api/titles/${env().ids['Начало']}`))).json();
  await Promise.all(Array.from({ length: 16 }, async () => {
    const response = await api.get(url(`/api/files/${title.files[0].id}/raw`), { headers: { Range: 'bytes=0-65535' } });
    expect(response.status()).toBe(206);
    expect((await response.body()).length).toBe(65536);
    await response.dispose();
  }));
  expect((await api.get(url('/api/health'))).ok()).toBe(true);
});
