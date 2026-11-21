const ESCROW_HEALTH_WARNING = 'EscrowHealthWarning';

function createEscrowHealthAlert({
  webhookUrl = process.env.ESCROW_HEALTH_ALERT_WEBHOOK_URL,
  fetchImpl = globalThis.fetch,
  logger = console.error
} = {}) {
  return async function alertEscrowHealthWarning(event) {
    const alert = {
      event: ESCROW_HEALTH_WARNING,
      warning_code: event.warning_type,
      invoice_id: event.invoice_id
    };

    logger('Escrow health warning', alert);

    if (!webhookUrl) return;
    if (typeof fetchImpl !== 'function') {
      throw new Error('Webhook delivery requires a Node.js runtime with fetch support');
    }

    const response = await fetchImpl(webhookUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(alert)
    });

    if (!response.ok) {
      throw new Error(`Health alert webhook returned HTTP ${response.status}`);
    }
  };
}

module.exports = { createEscrowHealthAlert };
