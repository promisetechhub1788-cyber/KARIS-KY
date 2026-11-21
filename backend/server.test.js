const assert = require('node:assert/strict');
const { after, before, describe, it } = require('node:test');
const { createServer } = require('./server');

describe('Escrow health warning endpoint', () => {
  let server;
  let baseUrl;
  const alerts = [];

  before(async () => {
    server = createServer({
      alertHealthWarning: async event => alerts.push(event.warning_type)
    });
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    baseUrl = `http://127.0.0.1:${server.address().port}`;
  });

  after(async () => {
    await new Promise((resolve, reject) => {
      server.close(error => error ? reject(error) : resolve());
    });
  });

  it('fires the alert hook with the non-zero warning code', async () => {
    const response = await fetch(`${baseUrl}/api/events/escrow-health-warning`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ warning_type: 4001, invoice_id: 'invoice-123' })
    });

    assert.equal(response.status, 202);
    assert.deepEqual(alerts, [4001]);
  });

  it('does not alert for warning code zero', async () => {
    const response = await fetch(`${baseUrl}/api/events/escrow-health-warning`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ warning_type: 0 })
    });

    assert.equal(response.status, 202);
    assert.deepEqual(alerts, [4001]);
  });
});
