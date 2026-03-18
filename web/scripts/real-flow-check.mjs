import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const baseUrl = process.env.PRISM_WEB_BASE_URL ?? 'http://127.0.0.1:3100';
const apiBaseUrl = process.env.PRISM_API_BASE_URL ?? 'http://127.0.0.1:8327';
const dashboardUsername = process.env.PRISM_DASHBOARD_USERNAME ?? 'admin';
const dashboardPassword = process.env.PRISM_DASHBOARD_PASSWORD ?? 'admin';
const ephemeralAuthKeyName = `e2e-temp-key-${Date.now().toString().slice(-6)}`;
const seedAuthKeyName = `e2e-seed-key-${Date.now().toString().slice(-6)}`;
const ephemeralProviderName = `e2e-provider-${Date.now().toString().slice(-6)}`;
const ephemeralProfileId = `e2e-profile-${Date.now().toString().slice(-4)}`;
const tenantId = 'e2e-tenant';

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(scriptPath);
const repoRoot = path.resolve(scriptDir, '..', '..');
const artifactRoot =
  process.env.PRISM_FLOW_ARTIFACTS_DIR ??
  process.env.PRISM_FLOW_OUTPUT_DIR ??
  path.join(repoRoot, 'artifacts', 'playwright', 'real-flow');
const runId = new Date().toISOString().replace(/\.\d{3}Z$/, 'Z').replaceAll(':', '-');
const runDir = path.join(artifactRoot, 'runs', runId);
const latestDir = path.join(artifactRoot, 'latest');

await fs.mkdir(runDir, { recursive: true });

function logStep(message) {
  console.error(`STEP ${message}`);
}

async function launchBrowser() {
  try {
    return await chromium.launch({ channel: 'chrome', headless: true });
  } catch {
    return chromium.launch({ headless: true });
  }
}

const browser = await launchBrowser();
const context = await browser.newContext({
  viewport: { width: 1600, height: 1200 },
});
const page = await context.newPage();
await page.addInitScript(() => {
  window.localStorage.setItem('prism-control-plane:locale', 'en-US');
});

const consoleEntries = [];
const failedRequests = [];
let dashboardCookieHeader = '';
let seedAuthKeyId = null;

page.on('console', (message) => {
  consoleEntries.push({
    type: message.type(),
    text: message.text(),
    url: page.url(),
  });
});

page.on('pageerror', (error) => {
  consoleEntries.push({
    type: 'pageerror',
    text: error.message,
    url: page.url(),
  });
});

page.on('response', (response) => {
  const responseUrl = response.url();
  if (response.status() >= 400) {
    const allowUnauthenticatedSessionProbe =
      response.status() === 401 &&
      responseUrl.includes('/api/dashboard/auth/session') &&
      page.url().includes('/login');

    if (!allowUnauthenticatedSessionProbe) {
      failedRequests.push({
        url: responseUrl,
        status: response.status(),
        route: page.url(),
      });
    }
  }
});

async function waitForStable() {
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(350);
}

async function acceptNextDialog() {
  page.once('dialog', async (dialog) => {
    await dialog.accept();
  });
}

async function capture(name) {
  await page.screenshot({
    path: path.join(runDir, `${name}.png`),
    fullPage: true,
  });
}

async function refreshLatestArtifacts() {
  await fs.rm(latestDir, { recursive: true, force: true });
  await fs.mkdir(latestDir, { recursive: true });
  const entries = await fs.readdir(runDir, { withFileTypes: true });
  await Promise.all(
    entries
      .filter((entry) => entry.isFile())
      .map((entry) =>
        fs.copyFile(path.join(runDir, entry.name), path.join(latestDir, entry.name)),
      ),
  );
}

async function expectNoBlockingError(route) {
  const body = await page.locator('body').innerText();
  assert(!body.includes('Failed to load control-plane workspace'), `${route} shows workspace load error`);
  assert(!body.includes('Unauthorized'), `${route} shows unauthorized state`);
}

async function clickWorkspace(name, heading) {
  await page.getByRole('link', { name: new RegExp(name, 'i') }).click();
  await page.getByRole('heading', { name: heading, exact: true }).waitFor({ timeout: 10_000 });
  await waitForStable();
}

async function selectContext(label, value) {
  await page.getByLabel(label).selectOption(value);
  await waitForStable();
}

async function text(selector) {
  return (await page.locator(selector).textContent())?.trim() ?? '';
}

async function firstText(selector) {
  return (await page.locator(selector).first().textContent())?.trim() ?? '';
}

async function factValue(panelTitle, label) {
  const panel = page.locator('.panel').filter({ has: page.getByRole('heading', { name: panelTitle, exact: true }) });
  try {
    const row = panel.locator('li').filter({ has: page.getByText(label, { exact: true }) });
    return (await row.locator('strong').textContent({ timeout: 5_000 }))?.trim() ?? '';
  } catch {
    return (await panel.locator('li strong').first().textContent({ timeout: 5_000 }))?.trim() ?? '';
  }
}

async function closeWorkbench() {
  const closeButton = page.getByRole('button', { name: 'Close workbench' });
  if (await closeButton.isVisible()) {
    await closeButton.click();
    await page.waitForTimeout(250);
  }
}

function heroButton(name) {
  return page.locator('.hero-actions').getByRole('button', { name, exact: true });
}

function sheetActionButton(name) {
  return page.locator('.sheet__actions').getByRole('button', { name, exact: true });
}

function sheetSectionButton(sectionHeading, name) {
  return page
    .locator('.sheet-section')
    .filter({ has: page.getByRole('heading', { name: sectionHeading, exact: true }) })
    .getByRole('button', { name, exact: true });
}

function inspectorButton(name) {
  return page.locator('.inspector').getByRole('button', { name, exact: true });
}

async function login() {
  await page.goto(`${baseUrl}/login`, { waitUntil: 'domcontentloaded' });
  await page.getByRole('heading', { name: 'Enter the control plane', exact: true }).waitFor({ timeout: 10_000 });
  await page.getByLabel('Username').fill(dashboardUsername);
  await page.getByLabel('Password').fill(dashboardPassword);
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await page.getByRole('heading', { name: 'Operate from runtime posture, not page sprawl', exact: true }).waitFor({ timeout: 10_000 });
  await waitForStable();
  await expectNoBlockingError('/command-center');
}

async function ensureDashboardSession() {
  if (dashboardCookieHeader) {
    return dashboardCookieHeader;
  }

  const response = await fetch(`${apiBaseUrl}/api/dashboard/auth/login`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      username: dashboardUsername,
      password: dashboardPassword,
    }),
  });

  assert.equal(response.ok, true, `dashboard login failed: ${response.status} ${await response.text()}`);
  const cookie = response.headers.get('set-cookie');
  assert(cookie, 'dashboard login did not return a session cookie');
  dashboardCookieHeader = cookie.split(';')[0];
  return dashboardCookieHeader;
}

async function dashboardJson(pathname, init = {}) {
  const cookie = await ensureDashboardSession();
  const response = await fetch(`${apiBaseUrl}${pathname}`, {
    ...init,
    headers: {
      cookie,
      ...(init.body ? { 'Content-Type': 'application/json' } : {}),
      ...(init.headers ?? {}),
    },
  });

  const text = await response.text();
  const data = text ? JSON.parse(text) : null;
  assert.equal(response.ok, true, `dashboard request failed: ${response.status} ${text}`);
  return data;
}

async function cleanupEphemeralAuthKeys() {
  const response = await dashboardJson('/api/dashboard/auth-keys');
  const keys = response.auth_keys ?? [];
  const staleKeys = keys.filter((key) => typeof key.name === 'string' && key.name.startsWith('e2e-'));
  for (const key of staleKeys) {
    await dashboardJson(`/api/dashboard/auth-keys/${key.id}`, { method: 'DELETE' });
  }
}

async function createSeedAuthKey(tenantIdOverride) {
  await dashboardJson('/api/dashboard/auth-keys', {
    method: 'POST',
    body: JSON.stringify({
      name: seedAuthKeyName,
      ...(tenantIdOverride ? { tenant_id: tenantIdOverride } : {}),
    }),
  });
  const listResponse = await dashboardJson('/api/dashboard/auth-keys');
  const created = (listResponse.auth_keys ?? []).find((key) => key.name === seedAuthKeyName);
  assert(created, 'seed auth key was not created');
  seedAuthKeyId = created.id;
  const revealed = await dashboardJson(`/api/dashboard/auth-keys/${created.id}/reveal`, {
    method: 'POST',
  });
  return revealed.key;
}

async function deleteSeedAuthKey() {
  if (seedAuthKeyId === null) {
    return;
  }
  await dashboardJson(`/api/dashboard/auth-keys/${seedAuthKeyId}`, { method: 'DELETE' });
  seedAuthKeyId = null;
}

async function sendTrafficRequest(authKey) {
  const result = await fetch(`${apiBaseUrl}/api/provider/${encodeURIComponent('ChatGPT Pro')}/v1/responses`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${authKey}`,
    },
    body: JSON.stringify({
      model: 'gpt-5',
      input: 'Reply with the single word ok.',
    }),
  });

  const response = {
    ok: result.ok,
    status: result.status,
    body: await result.text(),
  };

  assert.equal(response.ok, true, `traffic seed request failed: ${response.status} ${response.body}`);
}

async function seedTrafficRequest(tenantIdOverride) {
  await cleanupEphemeralAuthKeys();
  const authKey = await createSeedAuthKey(tenantIdOverride);
  try {
    await sendTrafficRequest(authKey);
  } finally {
    await deleteSeedAuthKey();
  }
  await page.waitForTimeout(1200);
}

const report = {
  artifactRoot,
  latestDir,
  runDir,
  runId,
  routes: [],
  controls: [],
  actions: [],
  consoleEntries,
  failedRequests,
};

logStep('login');
await login();
await page.locator('.panel')
  .filter({ has: page.getByRole('heading', { name: 'Watch windows', exact: true }) })
  .locator('li')
  .first()
  .waitFor({ timeout: 20_000 });
await capture('command-center');

assert((await factValue('Watch windows', 'Latest request')).length > 0, 'command center watch window should contain values');
const signalCount = await page.locator('.signal-row').count();
report.routes.push({
  route: '/command-center',
  signalRows: signalCount,
  watchWindowLatestRequest: await factValue('Watch windows', 'Latest request'),
  systemStatus: await factValue('System watch', 'Status'),
});

logStep('context-controls');
await selectContext('Range', '15m');
report.controls.push({ control: 'Range', value: '15m' });
await selectContext('Source', 'runtime');
report.controls.push({
  control: 'Source',
  value: 'runtime',
  pill: await text('.source-posture .status-pill'),
});
await page.getByRole('button', { name: 'Live', exact: true }).click();
await waitForStable();
report.controls.push({
  control: 'Live',
  value: await page.getByRole('button', { name: /Paused|Live/ }).textContent(),
});
await page.getByRole('button', { name: '中文' }).click();
await waitForStable();
await page.getByText('控制平面', { exact: true }).waitFor({ timeout: 10_000 });
report.controls.push({
  control: 'Locale',
  value: 'zh-CN',
  heading: await page.getByText('控制平面', { exact: true }).textContent(),
});
await page.getByRole('button', { name: 'English' }).click();
await waitForStable();
await page.getByText('Control plane', { exact: true }).waitFor({ timeout: 10_000 });
report.controls.push({
  control: 'Locale',
  value: 'en-US',
  heading: await page.getByText('Control plane', { exact: true }).textContent(),
});

logStep('command-center-actions');
await heroButton('Diagnostics').click();
await page.getByRole('heading', { name: 'Diagnostics', exact: true }).waitFor({ timeout: 10_000 });
await page.getByLabel('Search').fill('Routing');
await page.getByRole('button', { name: 'Apply filters', exact: true }).click();
await page.getByRole('button', { name: 'Reload runtime', exact: true }).click();
await page.getByText(/Runtime reload completed/i).waitFor({ timeout: 20_000 });
report.actions.push({
  route: '/command-center',
  action: 'Diagnostics workbench',
  totalHits: await page.locator('.sheet .detail-grid__row').filter({ has: page.getByText('Total hits', { exact: true }) }).locator('strong').textContent(),
});
await closeWorkbench();

await heroButton('Command palette').click();
await page.getByRole('heading', { name: 'Command palette', exact: true }).waitFor({ timeout: 10_000 });
await page.getByRole('button', { name: 'Inspect provider roster', exact: true }).click();
await page.getByRole('heading', { name: 'Operate providers from runtime truth', exact: true }).waitFor({ timeout: 10_000 });
await waitForStable();
report.actions.push({
  route: '/command-center',
  action: 'Command palette',
  landedOn: page.url(),
});

await clickWorkspace('Command Center', 'Operate from runtime posture, not page sprawl');
if (signalCount > 0) {
  await heroButton('Open live investigation').click();
  await page.waitForURL((url) => url.pathname.endsWith('/traffic-lab'), { timeout: 10_000 });
  await waitForStable();
  report.actions.push({
    route: '/command-center',
    action: 'Open investigation',
    landedOn: page.url(),
  });
  await clickWorkspace('Command Center', 'Operate from runtime posture, not page sprawl');
}

logStep('seed-traffic');
await seedTrafficRequest();
logStep('traffic-lab');
await clickWorkspace('Traffic Lab', 'Investigate requests as sessions');
assert((await page.locator('.table-grid--sessions .table-grid__cell--strong').count()) >= 1, 'traffic lab should show request sessions');
const firstSessionId = await firstText('.table-grid--sessions .table-grid__cell--strong');
assert(firstSessionId.length > 0, 'traffic lab session id should not be blank');
await page.getByPlaceholder('Filter request sessions').fill(firstSessionId.slice(0, 8));
await waitForStable();
assert(page.url().includes('q='), 'traffic lab should persist session filter in the URL');
const compareSelect = page.getByLabel('Compare request');
const compareOptions = await compareSelect.locator('option').count();
if (compareOptions > 1) {
  await compareSelect.selectOption({ index: 1 });
  await waitForStable();
}
await heroButton('Save lens').click();
await page.getByText('Current request lens saved locally.', { exact: true }).waitFor({ timeout: 10_000 });
await heroButton('Replay with current draft').click();
await page.getByRole('heading', { name: 'Replay and explain', exact: true }).waitFor({ timeout: 10_000 });
await page.getByRole('heading', { name: 'Route explanation', exact: true }).waitFor({ timeout: 10_000 });
await capture('traffic-lab');
report.routes.push({
  route: '/traffic-lab',
  firstSessionId,
  traceSteps: await page.locator('.timeline-step').count(),
  compareMode: compareOptions > 1,
});
report.actions.push({
  route: '/traffic-lab',
  action: 'Replay with draft',
  winner: await page.locator('.sheet .detail-grid__row').filter({ has: page.getByText('Winner', { exact: true }) }).locator('strong').textContent(),
});
await closeWorkbench();
await heroButton('Inspect session').click();
await page.getByRole('heading', { name: 'Request detail', exact: true }).waitFor({ timeout: 10_000 });
await page.getByRole('heading', { name: 'Payloads', exact: true }).waitFor({ timeout: 10_000 });
report.actions.push({
  route: '/traffic-lab',
  action: 'Inspect selected session',
  requestPath: await page.locator('.sheet .detail-grid__row').filter({ has: page.getByText('Path', { exact: true }) }).locator('strong').textContent(),
});
await closeWorkbench();
await page.getByPlaceholder('Filter request sessions').fill('');
await waitForStable();

logStep('provider-atlas');
await clickWorkspace('Provider Atlas', 'Operate providers from runtime truth');
assert(
  await page.locator('.table-grid--providers .table-grid__cell--strong').filter({ hasText: /^ChatGPT Pro$/ }).first().isVisible(),
  'provider atlas should show real provider',
);
await inspectorButton('Run health probe').click();
await page.getByRole('heading', { name: 'Provider editor', exact: true }).waitFor({ timeout: 10_000 });
await page.getByText(/Health probe completed with status/i).waitFor({ timeout: 20_000 });
await closeWorkbench();
report.actions.push({
  route: '/provider-atlas',
  action: 'Inspector run health probe',
  landedOn: page.url(),
});
await heroButton('Health and test').click();
await page.getByRole('heading', { name: 'Provider editor', exact: true }).waitFor({ timeout: 10_000 });
await page.getByRole('heading', { name: 'Auth profiles', exact: true }).waitFor({ timeout: 10_000 });
await page.getByRole('heading', { name: 'Managed auth runtime', exact: true }).waitFor({ timeout: 10_000 });
await sheetSectionButton('Live operations', 'Run health probe').click();
await page.getByText(/Health probe completed with status/i).waitFor({ timeout: 20_000 });
await sheetActionButton('Save provider').click();
await page.getByText(/Saved provider/i).waitFor({ timeout: 20_000 });
await capture('provider-atlas');
report.routes.push({
  route: '/provider-atlas',
  providerRows: await page.locator('.table-grid--providers .table-grid__cell--strong').count(),
  provider: await firstText('.table-grid--providers .table-grid__cell--strong'),
});
report.actions.push({
  route: '/provider-atlas',
  action: 'Provider editor',
  healthStatus: await page.locator('.sheet .detail-grid__row').filter({ has: page.getByText('Status', { exact: true }) }).locator('strong').first().textContent(),
});
await closeWorkbench();
report.actions.push({
  route: '/provider-atlas',
  action: 'Protocol and model truth',
  publicRoutes: await factValue('Protocol surfaces', 'Public routes'),
  modelEntry: await firstText('.panel .probe-check span'),
});
await page.getByPlaceholder('Filter protocol surfaces').fill('responses');
await page.getByPlaceholder('Filter model inventory').fill('gpt');
await waitForStable();

await page.getByRole('button', { name: 'Provider registry', exact: true }).click();
await page.getByRole('heading', { name: 'Provider registry workbench', exact: true }).waitFor({ timeout: 10_000 });
await page.getByLabel('Name').fill(ephemeralProviderName);
await page.getByLabel('API key').fill('sk-test-e2e-provider');
await page.getByLabel('Models').fill('gpt-4o-mini');
await sheetActionButton('Create provider').click();
await page.getByText(new RegExp(`Created provider ${ephemeralProviderName}`)).waitFor({ timeout: 20_000 });
await closeWorkbench();
await page.locator('.table-grid--providers .table-grid__cell--strong').filter({ hasText: new RegExp(`^${ephemeralProviderName}$`) }).first().waitFor({ timeout: 20_000 });
report.actions.push({
  route: '/provider-atlas',
  action: 'Create provider',
  provider: ephemeralProviderName,
});

await heroButton('Auth profile workbench').click();
await page.getByRole('heading', { name: 'Auth profile workbench', exact: true }).waitFor({ timeout: 10_000 });
await page.getByLabel('Provider').selectOption(ephemeralProviderName);
await page.getByLabel('Profile id').fill(ephemeralProfileId);
await page.getByLabel('Secret').fill('sk-test-e2e-profile');
await sheetActionButton('Create profile').click();
await page.getByText(new RegExp(`Created auth profile ${ephemeralProviderName}/${ephemeralProfileId}`)).waitFor({ timeout: 20_000 });
await page.locator('.sheet .probe-check').filter({ hasText: `${ephemeralProviderName}/${ephemeralProfileId}` }).getByRole('button', { name: 'Select', exact: true }).click();
await page.getByLabel('Prefix').fill('e2e-prefix');
await sheetActionButton('Save profile').click();
await page.getByText(new RegExp(`Saved auth profile ${ephemeralProviderName}/${ephemeralProfileId}`)).waitFor({ timeout: 20_000 });
await acceptNextDialog();
await sheetSectionButton('Selected profile actions', 'Delete selected').click();
await page.getByText(new RegExp(`Deleted auth profile ${ephemeralProviderName}/${ephemeralProfileId}`)).waitFor({ timeout: 20_000 });
report.actions.push({
  route: '/provider-atlas',
  action: 'Auth profile lifecycle',
  profile: `${ephemeralProviderName}/${ephemeralProfileId}`,
});
await closeWorkbench();
await page
  .locator('.table-grid--providers .table-grid__cell--strong')
  .filter({ hasText: new RegExp(`^${ephemeralProviderName}$`) })
  .first()
  .click();
await waitForStable();

await page.getByRole('button', { name: 'Provider registry', exact: true }).click();
await page.getByRole('heading', { name: 'Provider registry workbench', exact: true }).waitFor({ timeout: 10_000 });
await acceptNextDialog();
await sheetActionButton('Delete selected').click();
await page.getByText(new RegExp(`Deleted provider ${ephemeralProviderName}`)).waitFor({ timeout: 20_000 });
await closeWorkbench();
report.actions.push({
  route: '/provider-atlas',
  action: 'Delete provider',
  provider: ephemeralProviderName,
});

logStep('route-studio');
await clickWorkspace('Route Studio', 'Author routing from runtime truth');
const scenarioPanel = page.locator('.panel').filter({ has: page.getByRole('heading', { name: 'Scenario matrix', exact: true }) });
assert((await scenarioPanel.locator('.table-grid__cell--strong').count()) >= 1, 'route studio should show scenarios');
const firstScenario = await scenarioPanel.locator('.table-grid__cell--strong').first().textContent();
await page.getByRole('button', { name: 'New rule', exact: true }).click();
await page.getByLabel('Name').fill(`e2e-route-rule-${Date.now().toString().slice(-5)}`);
await page.getByLabel('Models').fill(`__never__-${Date.now().toString().slice(-5)}`);
await heroButton('Save draft').click();
await page.getByText(/Routing configuration updated successfully|Routing draft saved/i).waitFor({ timeout: 20_000 });
await heroButton('Simulate draft').click();
await page.getByRole('heading', { name: 'Simulation result', exact: true }).waitFor({ timeout: 10_000 });
await page.getByRole('heading', { name: 'Winning route', exact: true }).waitFor({ timeout: 10_000 });
await capture('route-studio');
report.routes.push({
  route: '/route-studio',
  scenarioRows: await scenarioPanel.locator('.table-grid__cell--strong').count(),
  firstScenario: firstScenario?.trim() ?? '',
});
await closeWorkbench();
await page.getByRole('button', { name: 'Delete selected', exact: true }).click();
await heroButton('Save draft').click();
await page.getByText(/Routing configuration updated successfully|Routing draft saved/i).waitFor({ timeout: 20_000 });
await heroButton('Simulate draft').click();
await page.getByRole('heading', { name: 'Simulation result', exact: true }).waitFor({ timeout: 10_000 });
await sheetActionButton('Promote').click();
await page.getByRole('heading', { name: 'Operate config as a transaction surface', exact: true }).waitFor({ timeout: 10_000 });
await waitForStable();
report.actions.push({
  route: '/route-studio',
  action: 'Promote to change',
  landedOn: page.url(),
});

logStep('change-studio');
assert((await page.locator('.table-grid--changes .table-grid__cell--strong').count()) >= 1, 'change studio should show registry rows');
await inspectorButton('Validate current config').click();
await page.getByRole('heading', { name: 'Structured change workbench', exact: true }).waitFor({ timeout: 10_000 });
await page
  .locator('.status-message')
  .filter({ hasText: /^Validation passed\.|^Validation returned issues\.$/ })
  .first()
  .waitFor({ timeout: 20_000 });
await closeWorkbench();
report.actions.push({
  route: '/change-studio',
  action: 'Inspector validate current config',
  landedOn: page.url(),
});
await heroButton('Create structured change').click();
await page.getByRole('heading', { name: 'Structured change workbench', exact: true }).waitFor({ timeout: 10_000 });
await page.getByRole('heading', { name: 'Linked route draft', exact: true }).waitFor({ timeout: 10_000 });
await sheetActionButton('Validate').click();
await page
  .locator('.status-message')
  .filter({ hasText: /^Validation passed\.|^Validation returned issues\.$/ })
  .first()
  .waitFor({ timeout: 20_000 });
await sheetActionButton('Apply draft').click();
await page.locator('.status-message').filter({ hasText: /^Applied config version / }).first().waitFor({ timeout: 20_000 });
await sheetActionButton('Reload runtime').click();
await page.locator('.status-message').filter({ hasText: /^Runtime reload completed\.$/ }).first().waitFor({ timeout: 20_000 });
await capture('change-studio');
report.routes.push({
  route: '/change-studio',
  registryRows: await page.locator('.table-grid--changes .table-grid__cell--strong').count(),
  firstFamily: await firstText('.table-grid--changes .table-grid__cell--strong'),
});
report.actions.push({
  route: '/change-studio',
  action: 'Structured change workbench',
  linkedRouteDraft: await page.locator('.sheet .detail-grid__row').filter({ has: page.getByText('Scenario', { exact: true }) }).locator('strong').first().textContent(),
});
await closeWorkbench();

await heroButton('Manage access keys').click();
await page.getByRole('heading', { name: 'Access control workbench', exact: true }).waitFor({ timeout: 10_000 });
await sheetActionButton('New draft').click();
await page.getByLabel('Name').fill(ephemeralAuthKeyName);
await page.getByLabel('Tenant ID').fill(tenantId);
await sheetActionButton('Create key').click();
await page.getByText(/Revealed now:/i).waitFor({ timeout: 20_000 });
await page.locator('.sheet .probe-check').filter({ hasText: ephemeralAuthKeyName }).first().waitFor({ timeout: 20_000 });
await page.locator('.sheet .probe-check').filter({ hasText: ephemeralAuthKeyName }).getByRole('button', { name: 'Select', exact: true }).click();
const createdKeyNotice = await page.locator('.status-message--warning').filter({ hasText: /Revealed now:/i }).textContent();
const createdAuthKey = createdKeyNotice?.split('Revealed now:')[1]?.trim() ?? '';
assert(createdAuthKey.length > 0, 'created auth key should be visible in the workbench');
await sendTrafficRequest(createdAuthKey);
await page.getByLabel('Allowed models').fill('gpt-5, gpt-4o-mini');
await sheetActionButton('Save key').click();
await page.getByText(new RegExp(`Saved auth key ${ephemeralAuthKeyName}`)).waitFor({ timeout: 20_000 });
await sheetSectionButton('Selected key actions', 'Reveal selected').click();
await page.getByText(/Revealed auth key/i).waitFor({ timeout: 20_000 });
await sheetSectionButton('Selected key actions', 'Delete selected').click();
await page.getByText(/Deleted auth key/i).waitFor({ timeout: 20_000 });
await closeWorkbench();
await page.getByRole('button', { name: 'Refresh posture', exact: true }).click();
await waitForStable();
await page.locator('.panel')
  .filter({ has: page.getByRole('heading', { name: 'Tenant posture', exact: true }) })
  .locator('.fact-list li')
  .filter({ hasText: tenantId })
  .click();
await page.locator('.detail-grid__row').filter({ has: page.getByText('Tenant', { exact: true }) }).locator('strong').first().waitFor({ timeout: 10_000 });
report.actions.push({
  route: '/change-studio',
  action: 'Access key lifecycle',
  keyName: ephemeralAuthKeyName,
  tenantId,
});
await closeWorkbench();

logStep('finalize');
await clickWorkspace('Command Center', 'Operate from runtime posture, not page sprawl');
await capture('command-center-final');
await browser.close();

const hardConsoleFailures = consoleEntries.filter((entry) =>
  ['error', 'pageerror'].includes(entry.type),
);

assert.equal(
  hardConsoleFailures.length,
  0,
  `console errors found: ${hardConsoleFailures.map((entry) => entry.text).join(' | ')}`,
);
assert.equal(
  failedRequests.length,
  0,
  `HTTP failures found: ${failedRequests.map((entry) => `${entry.status} ${entry.url}`).join(' | ')}`,
);

await fs.writeFile(
  path.join(runDir, 'report.json'),
  `${JSON.stringify(report, null, 2)}\n`,
  'utf8',
);

await refreshLatestArtifacts();

console.log(JSON.stringify(report, null, 2));
