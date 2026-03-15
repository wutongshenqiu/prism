import { test, expect } from '@playwright/test';

const providersPayload = {
  providers: [
    {
      name: 'openai-prod',
      format: 'openai',
      upstream: 'codex',
      api_key_masked: 'sk-o****prod',
      base_url: 'https://chatgpt.com/backend-api/codex',
      models: [{ id: 'gpt-5', alias: null }],
      disabled: false,
      wire_api: 'responses',
      upstream_presentation: { profile: 'codex-cli' },
      auth_profiles: [
        {
          id: 'codex-user',
          qualified_name: 'openai-prod/codex-user',
          mode: 'codex-oauth',
          header: 'bearer',
          refresh_token_present: true,
          id_token_present: true,
          email: 'codex@example.com',
          disabled: false,
          weight: 1,
        },
      ],
    },
  ],
};

const providerDetail = {
  name: 'openai-prod',
  format: 'openai',
  upstream: 'codex',
  api_key_masked: 'sk-o****prod',
  base_url: 'https://chatgpt.com/backend-api/codex',
  proxy_url: null,
  prefix: null,
  models: [{ id: 'gpt-5', alias: null }],
  excluded_models: [],
  headers: {},
  disabled: false,
  wire_api: 'responses',
  weight: 1,
  region: null,
  upstream_presentation: {
    profile: 'codex-cli',
    mode: 'always',
    'strict-mode': false,
    'sensitive-words': [],
    'cache-user-id': false,
    'custom-headers': {},
  },
  auth_profiles: [
    {
      id: 'codex-user',
      qualified_name: 'openai-prod/codex-user',
      mode: 'codex-oauth',
      header: 'bearer',
      refresh_token_present: true,
      id_token_present: true,
      email: 'codex@example.com',
      disabled: false,
      weight: 1,
    },
  ],
};

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    class MockWebSocket {
      static CONNECTING = 0;
      static OPEN = 1;
      static CLOSING = 2;
      static CLOSED = 3;
      readyState = MockWebSocket.OPEN;
      onopen: ((event: Event) => void) | null = null;
      onmessage: ((event: MessageEvent<string>) => void) | null = null;
      onclose: ((event: CloseEvent) => void) | null = null;
      onerror: ((event: Event) => void) | null = null;

      constructor() {
        queueMicrotask(() => this.onopen?.(new Event('open')));
      }

      close() {
        this.readyState = MockWebSocket.CLOSED;
      }

      send() {}
    }

    Object.defineProperty(window, 'WebSocket', {
      configurable: true,
      writable: true,
      value: MockWebSocket,
    });
  });

  await page.route('**/api/dashboard/providers', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: providersPayload });
      return;
    }
    await route.fulfill({ status: 200, json: { message: 'ok' } });
  });

  await page.route('**/api/dashboard/auth/session', async (route) => {
    await route.fulfill({ json: { authenticated: true, username: 'playwright' } });
  });

  await page.route('**/api/dashboard/providers/openai-prod', async (route) => {
    await route.fulfill({ json: providerDetail });
  });
});

test('providers page renders auth profile summary', async ({ page }) => {
  await page.goto('/providers');

  await expect(page.getByRole('heading', { name: 'Providers' })).toBeVisible();
  await expect(page.getByText('openai-prod')).toBeVisible();
  await expect(page.getByText('Auth profiles: codex-user')).toBeVisible();
  await expect(page.getByText('Codex CLI')).toBeVisible();
});

test('providers edit modal shows managed auth profile warning', async ({ page }) => {
  await page.goto('/providers');
  await page.locator('button[title="Edit"]').first().click();

  await expect(page.getByText('This provider uses managed auth profiles')).toBeVisible();
  await expect(page.getByText('codex-user · Codex OAuth')).toBeVisible();
  await expect(
    page.getByText(
      'Shared provider fields can be edited here. Credential material is managed from the dedicated Auth Profiles page.'
    )
  ).toBeVisible();
  await expect(page.getByRole('button', { name: /manage auth profiles/i })).toBeVisible();
});
