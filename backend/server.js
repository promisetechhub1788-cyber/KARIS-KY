const http = require('http');
const { createEscrowHealthAlert } = require('./health-alerts');

const HOST = '127.0.0.1';
const PORT = 8000;
const ENTRYPOINTS = contractSpec.entrypoints.map((entrypoint) => entrypoint.name);
const DEFAULT_ALLOWED_ORIGINS = ['http://127.0.0.1:5500', 'http://localhost:5500'];
const CONTRACT_ID_PATTERN = /^C[A-Z2-7]{55}$/;

function sendJson(res, status, payload) {
  res.writeHead(status, {
    'Content-Type': 'application/json',
    'Access-Control-Allow-Origin': '*'
  });
  res.end(JSON.stringify(payload));
}

function readJson(req) {
  return new Promise((resolve, reject) => {
    let body = '';
    req.setEncoding('utf8');

    req.on('data', chunk => {
      body += chunk;
      if (body.length > 1024 * 1024) {
        reject(new Error('Request body exceeds 1 MiB'));
        req.destroy();
      }
    });
    req.on('end', () => {
      try {
        resolve(JSON.parse(body));
      } catch {
        reject(new Error('Request body must be valid JSON'));
      }
    });
    req.on('error', reject);
  });
}

function createServer({ alertHealthWarning = createEscrowHealthAlert() } = {}) {
  return http.createServer(async (req, res) => {
    const url = new URL(req.url, `http://${req.headers.host}`);

    if (req.method === 'GET' && url.pathname === '/api/health') {
      sendJson(res, 200, {
        status: 'ok',
        service: 'karis-ky-backend',
        contracts: ['escrow']
      });
      return;
    }

    if (req.method === 'GET' && url.pathname === '/api/contract-summary') {
      sendJson(res, 200, {
        name: 'karis-ky Escrow Contracts',
        description: 'Soroban smart contracts for invoice liquidity on Stellar.',
        entrypoints: ['init', 'fund', 'settle', 'withdraw']
      });
      return;
    }

    if (req.method === 'POST' && url.pathname === '/api/events/escrow-health-warning') {
      let event;
      try {
        event = await readJson(req);
      } catch (error) {
        sendJson(res, 400, { error: error.message });
        return;
      }

      if (
        !event ||
        !Number.isInteger(event.warning_type) ||
        event.warning_type < 0 ||
        event.warning_type > 0xffffffff
      ) {
        sendJson(res, 400, { error: 'warning_type must be a uint32' });
        return;
      }

      if (event.warning_type === 0) {
        sendJson(res, 202, { accepted: true, alerted: false });
        return;
      }

      try {
        await alertHealthWarning(event);
      } catch (error) {
        console.error('Failed to deliver escrow health warning alert:', error);
        sendJson(res, 502, { error: 'failed to deliver health warning alert' });
        return;
      }

      sendJson(res, 202, { accepted: true, alerted: true });
      return;
    }

    sendJson(res, 404, { error: 'not found' });
  });
}

if (require.main === module) {
  const server = createServer();
  server.listen(PORT, HOST, () => {
    console.log(`Backend listening on http://${HOST}:${PORT}`);
  });
}

module.exports = { createServer };
