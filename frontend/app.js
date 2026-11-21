async function loadStatus() {
  const statusEl = document.getElementById('status');

  try {
    const response = await fetch('http://127.0.0.1:8000/api/health');
    const payload = await response.json();
    statusEl.replaceChildren();
    const contracts = Array.isArray(payload.contracts) ? payload.contracts.join(', ') : '';
    for (const [label, value] of [
      ['Backend status:', payload.status],
      ['Service:', payload.service],
      ['Contracts:', contracts]
    ]) {
      const strong = document.createElement('strong');
      strong.textContent = label;
      statusEl.append(strong, document.createTextNode(` ${String(value ?? '')}`));
      if (label !== 'Contracts:') statusEl.append(document.createElement('br'));
    }
  } catch (error) {
    statusEl.textContent = `Unable to connect to backend: ${String(error.message ?? error)}`;
  }
}

loadStatus();
