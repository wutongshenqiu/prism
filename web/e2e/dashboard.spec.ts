import { test, expect, type APIRequestContext, type Page } from '@playwright/test';

const FRONTEND = process.env.PRISM_FRONTEND_URL || 'http://127.0.0.1:4173';
const BACKEND = process.env.PRISM_BASE_URL || 'http://127.0.0.1:18317';
const USERNAME = process.env.PRISM_DASHBOARD_USERNAME || 'playwright';
const PASSWORD = process.env.PRISM_DASHBOARD_PASSWORD || 'test123';
const PROXY_KEY = process.env.PRISM_PROXY_KEY || 'sk-proxy-playwright';

async function login(page: Page) {
  await page.goto(`${FRONTEND}/login`);
  await page.waitForSelector('#username', { timeout: 10000 });
  await page.fill('#username', USERNAME);
  await page.fill('#password', PASSWORD);
  await page.click('button[type="submit"]');
  await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 15000 });
}

async function loginApi(request: APIRequestContext) {
  const response = await request.post(`${BACKEND}/api/dashboard/auth/login`, {
    data: { username: USERNAME, password: PASSWORD },
  });
  expect(response.ok()).toBeTruthy();
}

test.describe('Dashboard Login', () => {
  test('shows login page', async ({ page }) => {
    await page.goto(`${FRONTEND}/login`);
    await expect(page.getByRole('heading', { name: /Prism Gateway/i })).toBeVisible({ timeout: 10000 });
    await expect(page.locator('#username')).toBeVisible();
    await expect(page.locator('#password')).toBeVisible();
  });

  test('rejects wrong credentials', async ({ page }) => {
    await page.goto(`${FRONTEND}/login`);
    await page.waitForSelector('#username', { timeout: 10000 });
    await page.fill('#username', USERNAME);
    await page.fill('#password', 'wrongpassword');
    await page.click('button[type="submit"]');
    await expect(page.locator('.login-error')).toBeVisible({ timeout: 10000 });
  });

  test('successful login navigates to dashboard', async ({ page }) => {
    await login(page);
    await expect(page.locator('.metric-card')).toHaveCount(6);
  });
});

test.describe('Dashboard Pages', () => {
  test.beforeEach(async ({ page }) => {
    await login(page);
  });

  test('overview page shows metric cards', async ({ page }) => {
    await expect(page.locator('.metric-card').first()).toBeVisible({ timeout: 15000 });
    await expect(page.locator('body')).toContainText(/Total Requests|Success Rate|Avg Latency/i);
  });

  test('providers page renders managed auth profile summary', async ({ page }) => {
    await page.getByRole('link', { name: /providers/i }).click();
    await expect(page.getByRole('heading', { name: 'Providers' })).toBeVisible();
    await expect(page.locator('body')).toContainText('codex-gateway');
    await expect(page.getByTestId('provider-auth-profiles-codex-gateway')).toContainText('Auth profiles: codex-user');
  });

  test('editing a managed provider shows warning and presentation preview', async ({ page }) => {
    await page.getByRole('link', { name: /providers/i }).click();
    await page.getByTitle('Edit').first().click();
    await expect(page.getByText('This provider uses managed auth profiles')).toBeVisible();
    await expect(page.locator('.modal-body')).toContainText('codex-user');
    await page.getByRole('button', { name: /preview presentation/i }).click();
    await expect(page.locator('.modal-body')).toContainText(/Profile:\s+codex-cli/i);
    await expect(page.locator('.modal-body')).toContainText(/user-agent:\s+Codex\/1.0.0/i);
    await page.getByRole('button', { name: /manage auth profiles/i }).click();
    await expect(page).toHaveURL(/\/auth-profiles\?provider=codex-gateway/);
    await expect(page.getByRole('heading', { name: 'Auth Profiles' })).toBeVisible();
    await expect(page.getByTestId('auth-profile-row-codex-gateway/codex-user')).toBeVisible();
  });

  test('providers page can create and delete a legacy api-key provider', async ({ page }) => {
    const providerName = `playwright-temp-${Date.now()}`;
    page.once('dialog', (dialog) => dialog.accept());

    await page.getByRole('link', { name: /providers/i }).click();
    await page.getByRole('button', { name: /add provider/i }).click();
    await page.locator('input[placeholder="e.g., deepseek, openai-prod"]').fill(providerName);
    await page.locator('input[placeholder="sk-..."]').fill('sk-temp-provider');
    await page.locator('input[placeholder="gpt-4o, gpt-4o-mini, gpt-3.5-turbo"]').fill('gpt-4.1-mini');
    await page.getByRole('button', { name: /^create$/i }).click();

    await expect(page.locator('body')).toContainText(`Provider "${providerName}" created successfully.`);
    await expect(page.locator('tr', { hasText: providerName })).toBeVisible();

    const row = page.locator('tr', { hasText: providerName });
    await row.getByTitle('Delete').click();

    await expect(page.locator('body')).toContainText(`Provider "${providerName}" deleted successfully.`);
    await expect(page.locator('tr', { hasText: providerName })).toHaveCount(0);
  });

  test('request logs page loads with empty-state friendly copy', async ({ page }) => {
    await page.getByRole('link', { name: /requests/i }).click();
    await expect(page.getByRole('heading', { name: 'Request Logs' })).toBeVisible();
    await expect(page.locator('body')).toContainText(/0 total requests|No request logs/i);
  });

  test('auth profiles page can create, edit, and delete an api-key profile', async ({ page }) => {
    const profileId = `pw-api-${Date.now()}`;
    page.once('dialog', (dialog) => dialog.accept());

    await page.getByRole('link', { name: /auth profiles/i }).click();
    await expect(page.getByRole('heading', { name: 'Auth Profiles' })).toBeVisible();

    await page.getByRole('button', { name: /add auth profile/i }).click();
    await page.locator('.modal select').first().selectOption('claude-gateway');
    await page.getByPlaceholder('e.g. billing, oauth-user').fill(profileId);
    await page.getByPlaceholder('sk-...').fill('sk-auth-profile-playwright');
    await page.getByRole('button', { name: /^create$/i }).click();

    await expect(page.locator('body')).toContainText(`Auth profile "claude-gateway/${profileId}" created.`);
    const row = page.getByTestId(`auth-profile-row-claude-gateway/${profileId}`);
    await expect(row).toBeVisible();
    await expect(row).toContainText('Connected');

    await row.getByTitle('Edit').click();
    await page.locator('.modal input[type="number"]').fill('3');
    await page.getByPlaceholder('optional').nth(1).fill('claude/');
    await page.getByRole('button', { name: /^update$/i }).click();

    await expect(page.locator('body')).toContainText(`Auth profile "claude-gateway/${profileId}" updated.`);
    await expect(row).toContainText('weight 3');
    await expect(row).toContainText('claude/');

    await row.getByTitle('Delete').click();
    await expect(page.locator('body')).toContainText(`Auth profile "claude-gateway/${profileId}" deleted.`);
    await expect(page.getByTestId(`auth-profile-row-claude-gateway/${profileId}`)).toHaveCount(0);
  });

  test('auth profiles page can connect, refresh, and delete a codex oauth profile', async ({ page }) => {
    const profileId = `pw-oauth-${Date.now()}`;
    page.once('dialog', (dialog) => dialog.accept());

    await page.getByRole('link', { name: /auth profiles/i }).click();
    await page.getByRole('button', { name: /add auth profile/i }).click();
    await page.locator('.modal select').first().selectOption('codex-gateway');
    await page.getByPlaceholder('e.g. billing, oauth-user').fill(profileId);
    await page.locator('.modal select').nth(1).selectOption('codex-oauth');
    await page.getByRole('button', { name: /^create$/i }).click();

    await expect(page.locator('body')).toContainText(`Auth profile "codex-gateway/${profileId}" created.`);
    let row = page.getByTestId(`auth-profile-row-codex-gateway/${profileId}`);
    await expect(row).toContainText('Disconnected');

    await row.getByTitle('Connect OAuth').click();
    await expect(page.getByRole('heading', { name: 'Connect Auth Profile' })).toBeVisible();
    await Promise.all([
      page.waitForURL(/\/auth-profiles\/callback/),
      page.getByRole('button', { name: /open browser oauth/i }).click(),
    ]);
    await page.waitForURL((url) => url.pathname === '/auth-profiles');
    await expect(page.getByRole('heading', { name: 'Auth Profiles' })).toBeVisible();
    await expect(page.locator('body')).toContainText(`OAuth login completed for ${profileId}.`);

    row = page.getByTestId(`auth-profile-row-codex-gateway/${profileId}`);
    await expect(row).toContainText('Connected');
    await expect(row).toContainText('oauth-playwright@example.com');

    await row.getByTitle('Refresh token').click();
    await expect(page.locator('body')).toContainText(`Auth profile "codex-gateway/${profileId}" refreshed.`);
    await expect(row).toContainText('Connected');

    await row.getByTitle('Delete').click();
    await expect(page.locator('body')).toContainText(`Auth profile "codex-gateway/${profileId}" deleted.`);
    await expect(page.getByTestId(`auth-profile-row-codex-gateway/${profileId}`)).toHaveCount(0);
  });

  test('auth profiles page can connect and delete a claude subscription profile', async ({ page }) => {
    const profileId = `pw-claude-sub-${Date.now()}`;
    const setupToken = `sk-ant-oat01-${'a'.repeat(96)}`;
    page.once('dialog', (dialog) => dialog.accept());

    await page.getByRole('link', { name: /auth profiles/i }).click();
    await page.getByRole('button', { name: /add auth profile/i }).click();
    await page.locator('.modal select').first().selectOption('claude-gateway');
    await page.getByPlaceholder('e.g. billing, oauth-user').fill(profileId);
    await page.locator('.modal select').nth(1).selectOption('anthropic-claude-subscription');
    await page.getByRole('button', { name: /^create$/i }).click();

    await expect(page.locator('body')).toContainText(`Auth profile "claude-gateway/${profileId}" created.`);
    let row = page.getByTestId(`auth-profile-row-claude-gateway/${profileId}`);
    await expect(row).toContainText('Disconnected');

    await row.getByTitle('Connect token').click();
    await expect(page.getByRole('heading', { name: 'Connect Auth Profile' })).toBeVisible();
    await page.getByPlaceholder('sk-ant-oat01-...').fill(setupToken);
    await page.getByRole('button', { name: /^connect$/i }).click();

    await expect(page.locator('body')).toContainText(`Auth profile "claude-gateway/${profileId}" connected.`);
    row = page.getByTestId(`auth-profile-row-claude-gateway/${profileId}`);
    await expect(row).toContainText('Connected');
    await expect(row).toContainText('sk-a****aaaa');

    await row.getByTitle('Delete').click();
    await expect(page.locator('body')).toContainText(`Auth profile "claude-gateway/${profileId}" deleted.`);
    await expect(page.getByTestId(`auth-profile-row-claude-gateway/${profileId}`)).toHaveCount(0);
  });

  test('routing page loads', async ({ page }) => {
    await page.getByRole('link', { name: /routing/i }).click();
    await expect(page.getByRole('heading', { name: 'Routing', exact: true })).toBeVisible();
    await expect(page.locator('body')).toContainText(/balanced|lowest-cost|lowest-latency|stable/i);
  });

  test('replay page explains a live route', async ({ page }) => {
    await page.getByRole('link', { name: /replay/i }).click();
    await expect(page.getByRole('heading', { name: 'Replay' })).toBeVisible();

    await page.getByPlaceholder('e.g. gpt-4, claude-sonnet-4-5').fill('gpt-5');
    await page.getByRole('button', { name: /explain route/i }).click();

    await expect(page.locator('body')).toContainText('Route Selected');
    await expect(page.locator('body')).toContainText('codex-gateway');
    await expect(page.locator('body')).toContainText('gpt-5');
  });

  test('protocols page derives streaming support from live matrix', async ({ page }) => {
    await page.getByRole('link', { name: /protocols/i }).click();
    await expect(page.getByRole('heading', { name: 'Protocols' })).toBeVisible();
    await expect(page.locator('body')).toContainText('/v1/responses');
    await expect(page.locator('body')).toContainText('Streaming availability below is derived from the currently active provider set');
    await expect(page.locator('body')).toContainText(/codex-gateway|claude-gateway/);
  });

  test('models page shows runtime capability truth', async ({ page }) => {
    await page.getByRole('link', { name: /models & capabilities/i }).click();
    await expect(page.getByRole('heading', { name: 'Models & Capabilities' })).toBeVisible();
    await expect(page.locator('body')).toContainText('Provider Capabilities');
    await expect(page.locator('body')).toContainText('gpt-5');
    await expect(page.locator('body')).toContainText('claude-sonnet-4-20250514');
    await expect(
      page.getByRole('table').first().getByRole('row', { name: /codex-gateway openai/i }),
    ).toContainText(/Yes|Unknown|No/);
  });

  test('tenants page exposes tenants and embedded auth keys', async ({ page }) => {
    await page.getByRole('link', { name: /tenants & keys/i }).click();
    await expect(page.getByRole('heading', { name: 'Tenants & Keys' })).toBeVisible();
    await expect(page.locator('body')).toContainText(/No tenants found|playwright/i);

    await page.getByRole('button', { name: /^auth keys$/i }).click();
    await expect(page.getByRole('button', { name: /create key/i })).toBeVisible();
  });

  test('config page uses runtime snapshot and yaml workspace without fake schema tab', async ({ page }) => {
    await page.getByRole('link', { name: /config & changes/i }).click();
    await expect(page.getByRole('heading', { name: 'Config & Changes' })).toBeVisible();
    await expect(page.getByRole('button', { name: /runtime snapshot/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /config schema/i })).toHaveCount(0);
    await expect(page.locator('body')).toContainText('Sanitized Runtime Snapshot');
    await expect(page.locator('body')).toContainText('Provider Inventory');

    await page.getByRole('button', { name: /yaml editor/i }).click();
    await page.getByRole('button', { name: /^validate$/i }).click();
    await expect(page.locator('body')).toContainText('Configuration is valid');
  });

  test('system page shows health info', async ({ page }) => {
    await page.getByRole('link', { name: /system/i }).click();
    await expect(page.getByRole('heading', { name: 'System' })).toBeVisible();
    await expect(page.locator('body')).toContainText(/Uptime|Version|Provider Health/i);
    await expect(page.locator('body')).toContainText(/codex-gateway|claude-gateway/i);
  });
});

test.describe('Dashboard API', () => {
  test('dashboard auth-profiles endpoint returns nested auth profile state', async ({ request }) => {
    await loginApi(request);
    const resp = await request.get(`${BACKEND}/api/dashboard/auth-profiles`);
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.profiles).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          provider: 'codex-gateway',
          id: 'codex-user',
          mode: 'codex-oauth',
          refresh_token_present: true,
        }),
      ]),
    );
  });

  test('dashboard providers endpoint returns auth profile summaries', async ({ request }) => {
    await loginApi(request);
    const resp = await request.get(`${BACKEND}/api/dashboard/providers`);
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.providers).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          name: 'codex-gateway',
          auth_profiles: expect.arrayContaining([
            expect.objectContaining({ id: 'codex-user', mode: 'codex-oauth' }),
          ]),
        }),
      ]),
    );
  });

  test('codex oauth lifecycle endpoints start, complete, and refresh a managed auth profile', async ({ request }) => {
    await loginApi(request);

    const startResp = await request.post(`${BACKEND}/api/dashboard/auth-profiles/codex/oauth/start`, {
      data: {
        provider: 'codex-gateway',
        profile_id: 'codex-session',
        redirect_uri: 'http://127.0.0.1/callback',
      },
    });
    expect(startResp.status()).toBe(200);
    const startBody = await startResp.json();
    expect(startBody.profile_id).toBe('codex-session');
    expect(startBody.auth_url).toContain('playwright-codex-client');
    expect(startBody.state).toBeTruthy();

    const completeResp = await request.post(`${BACKEND}/api/dashboard/auth-profiles/codex/oauth/complete`, {
      data: {
        state: startBody.state,
        code: 'playwright-code',
      },
    });
    expect(completeResp.status()).toBe(200);
    const completeBody = await completeResp.json();
    expect(completeBody.profile).toEqual(
      expect.objectContaining({
        provider: 'codex-gateway',
        id: 'codex-session',
        mode: 'codex-oauth',
        refresh_token_present: true,
        email: 'oauth-playwright@example.com',
      }),
    );

    const refreshResp = await request.post(`${BACKEND}/api/dashboard/auth-profiles/codex-gateway/codex-session/refresh`, {
    });
    expect(refreshResp.status()).toBe(200);
    const refreshBody = await refreshResp.json();
    expect(refreshBody.profile).toEqual(
      expect.objectContaining({
        provider: 'codex-gateway',
        id: 'codex-session',
        mode: 'codex-oauth',
        refresh_token_present: true,
        account_id: 'acct_refresh',
      }),
    );
  });

  test('models endpoint exposes configured models for proxy keys', async ({ request }) => {
    const resp = await request.get(`${BACKEND}/v1/models`, {
      headers: { 'x-api-key': PROXY_KEY },
    });
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.data).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 'gpt-5' }),
        expect.objectContaining({ id: 'gpt-5-mini' }),
        expect.objectContaining({ id: 'sonnet' }),
      ]),
    );
  });

  test('health endpoint', async ({ request }) => {
    const resp = await request.get(`${BACKEND}/health`);
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.status).toBe('ok');
  });
});
