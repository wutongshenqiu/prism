import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';

const frontendUrl = process.env.PRISM_FRONTEND_URL || 'http://127.0.0.1:4173';
const backendUrl = process.env.PRISM_BASE_URL || 'http://127.0.0.1:18317';
const mockOauthPort = process.env.PRISM_MOCK_CODEX_OAUTH_PORT || '18418';
const mockOauthBaseUrl = `http://127.0.0.1:${mockOauthPort}`;
const skipLocalBackend = process.env.PRISM_SKIP_LOCAL_BACKEND === '1';
const configDir = fileURLToPath(new URL('.', import.meta.url));
const repoRoot = fileURLToPath(new URL('..', import.meta.url));

const localBackendServers = skipLocalBackend
  ? []
  : [
      {
        command: 'node ./web/e2e/mock-codex-oauth.mjs',
        url: `${mockOauthBaseUrl}/health`,
        cwd: repoRoot,
        reuseExistingServer: false,
        env: {
          ...process.env,
          PRISM_MOCK_CODEX_OAUTH_PORT: mockOauthPort,
        },
      },
      {
        command: './web/e2e/start-prism-e2e.sh',
        url: `${backendUrl}/health`,
        cwd: repoRoot,
        reuseExistingServer: false,
        env: {
          ...process.env,
          PRISM_BASE_URL: backendUrl,
          PRISM_CODEX_AUTH_URL: `${mockOauthBaseUrl}/oauth/authorize`,
          PRISM_CODEX_TOKEN_URL: `${mockOauthBaseUrl}/oauth/token`,
          PRISM_CODEX_CLIENT_ID: 'playwright-codex-client',
        },
      },
    ];

export default defineConfig({
  testDir: './e2e',
  timeout: 60_000,
  workers: 1,
  retries: 0,
  webServer: [
    ...localBackendServers,
    {
      command: 'npm run dev -- --host 127.0.0.1 --port 4173',
      url: frontendUrl,
      cwd: configDir,
      reuseExistingServer: false,
      env: {
        ...process.env,
        PRISM_BASE_URL: backendUrl,
        PRISM_FRONTEND_URL: frontendUrl,
      },
    },
  ],
  use: {
    baseURL: frontendUrl,
    headless: true,
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { browserName: 'chromium' },
    },
  ],
});
