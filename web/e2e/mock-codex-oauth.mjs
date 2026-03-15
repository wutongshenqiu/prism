import { createServer } from 'node:http';

const port = Number(process.env.PRISM_MOCK_CODEX_OAUTH_PORT || 18418);

function base64UrlEncode(value) {
  return Buffer.from(JSON.stringify(value)).toString('base64url');
}

function makeIdToken(email, accountId) {
  const header = base64UrlEncode({ alg: 'none', typ: 'JWT' });
  const payload = base64UrlEncode({
    email,
    sub: accountId,
    account_id: accountId,
  });
  return `${header}.${payload}.sig`;
}

function sendJson(res, status, body) {
  res.writeHead(status, { 'content-type': 'application/json' });
  res.end(JSON.stringify(body));
}

const server = createServer((req, res) => {
  if (!req.url) {
    sendJson(res, 404, { error: 'not_found' });
    return;
  }

  const url = new URL(req.url, `http://127.0.0.1:${port}`);

  if (req.method === 'GET' && url.pathname === '/health') {
    sendJson(res, 200, { status: 'ok' });
    return;
  }

  if (req.method === 'GET' && url.pathname === '/oauth/authorize') {
    const redirectUri = url.searchParams.get('redirect_uri');
    const state = url.searchParams.get('state');
    if (redirectUri && state) {
      const redirectUrl = new URL(redirectUri);
      redirectUrl.searchParams.set('code', 'playwright-code');
      redirectUrl.searchParams.set('state', state);
      res.writeHead(302, { location: redirectUrl.toString() });
      res.end();
      return;
    }
    sendJson(res, 200, { ok: true, state });
    return;
  }

  if (req.method === 'POST' && url.pathname === '/oauth/token') {
    let body = '';
    req.on('data', (chunk) => {
      body += chunk;
    });
    req.on('end', () => {
      const params = new URLSearchParams(body);
      const grantType = params.get('grant_type');

      if (grantType === 'authorization_code') {
        sendJson(res, 200, {
          access_token: 'codex-access-from-code',
          refresh_token: 'codex-refresh-from-code',
          id_token: makeIdToken('oauth-playwright@example.com', 'acct_code'),
          expires_in: 3600,
        });
        return;
      }

      if (grantType === 'refresh_token') {
        sendJson(res, 200, {
          access_token: 'codex-access-from-refresh',
          refresh_token: 'codex-refresh-from-refresh',
          id_token: makeIdToken('oauth-playwright@example.com', 'acct_refresh'),
          expires_in: 3600,
        });
        return;
      }

      sendJson(res, 400, { error: 'unsupported_grant_type', grant_type: grantType });
    });
    return;
  }

  sendJson(res, 404, { error: 'not_found', path: url.pathname });
});

server.listen(port, '127.0.0.1', () => {
  process.stdout.write(`mock codex oauth listening on ${port}\n`);
});
