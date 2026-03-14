import { test, expect } from '@playwright/test';

// Vite dev server serves the SPA and proxies /api/dashboard to Prism backend
const FRONTEND = process.env.PRISM_FRONTEND_URL || 'http://localhost:3000';
// Prism backend for direct API calls
const BACKEND = process.env.PRISM_BASE_URL || 'http://localhost:8317';
const USERNAME = 'admin';
const PASSWORD = 'test123';

async function login(page: import('@playwright/test').Page) {
  await page.goto(FRONTEND);
  await page.waitForSelector('#username', { timeout: 10000 });
  await page.fill('#username', USERNAME);
  await page.fill('#password', PASSWORD);
  await page.click('button[type="submit"]');
  // Wait for navigation away from login
  await page.waitForFunction(() => !document.querySelector('#username'), { timeout: 15000 });
}

test.describe('Dashboard Login', () => {
  test('shows login page', async ({ page }) => {
    await page.goto(FRONTEND);
    await expect(page.locator('h1')).toContainText('Prism', { timeout: 10000 });
    await expect(page.locator('#username')).toBeVisible();
    await expect(page.locator('#password')).toBeVisible();
  });

  test('rejects wrong credentials', async ({ page }) => {
    await page.goto(FRONTEND);
    await page.waitForSelector('#username', { timeout: 10000 });
    await page.fill('#username', 'admin');
    await page.fill('#password', 'wrongpassword');
    await page.click('button[type="submit"]');
    await expect(page.locator('.login-error')).toBeVisible({ timeout: 10000 });
  });

  test('successful login navigates to dashboard', async ({ page }) => {
    await login(page);
    await expect(page.locator('body')).toContainText(/Total Requests|Overview|Dashboard/i, { timeout: 15000 });
  });
});

test.describe('Dashboard Pages', () => {
  test.beforeEach(async ({ page }) => {
    await login(page);
  });

  test('overview page shows metric cards', async ({ page }) => {
    await expect(page.locator('.metric-card').first()).toBeVisible({ timeout: 15000 });
  });

  test('providers page lists providers', async ({ page }) => {
    await page.getByRole('link', { name: /providers/i }).click();
    await expect(page.locator('body')).toContainText(/bailian/i, { timeout: 15000 });
  });

  test('request logs page shows entries', async ({ page }) => {
    await page.getByRole('link', { name: /request/i }).click();
    await expect(page.locator('body')).toContainText(/qwen|Request Logs/i, { timeout: 15000 });
  });

  test('routing page loads', async ({ page }) => {
    await page.getByRole('link', { name: /routing/i }).click();
    await expect(page.locator('body')).toContainText(/routing|profile/i, { timeout: 15000 });
  });

  test('system page shows health info', async ({ page }) => {
    await page.getByRole('link', { name: /system/i }).click();
    await expect(page.locator('body')).toContainText(/uptime|version|system/i, { timeout: 15000 });
  });
});

test.describe('API E2E', () => {
  test('non-streaming chat completion', async ({ request }) => {
    const resp = await request.post(`${BACKEND}/v1/chat/completions`, {
      headers: { 'Content-Type': 'application/json' },
      data: {
        model: 'qwen3-coder-plus',
        messages: [{ role: 'user', content: 'Say pong' }],
        max_tokens: 10,
      },
    });
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.choices).toBeDefined();
    expect(body.choices[0].message.content).toBeTruthy();
  });

  test('streaming chat completion', async ({ request }) => {
    const resp = await request.post(`${BACKEND}/v1/chat/completions`, {
      headers: { 'Content-Type': 'application/json' },
      data: {
        model: 'qwen3-coder-plus',
        messages: [{ role: 'user', content: 'Say pong' }],
        max_tokens: 10,
        stream: true,
      },
    });
    expect(resp.status()).toBe(200);
    const text = await resp.text();
    expect(text).toContain('data:');
    expect(text).toContain('[DONE]');
  });

  test('models endpoint', async ({ request }) => {
    const resp = await request.get(`${BACKEND}/v1/models`);
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.data.some((m: any) => m.id === 'qwen3-coder-plus')).toBe(true);
  });

  test('health endpoint', async ({ request }) => {
    const resp = await request.get(`${BACKEND}/health`);
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.status).toBe('ok');
  });
});
