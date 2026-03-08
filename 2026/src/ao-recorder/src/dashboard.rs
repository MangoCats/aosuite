//! Dashboard endpoint: serves a static HTML page that auto-refreshes from /health.

use std::sync::Arc;

use axum::extract::State;
use axum::response::{Html, IntoResponse};

use crate::AppState;

/// GET /dashboard — static HTML operations dashboard.
pub async fn dashboard(
    State(_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    Html(DASHBOARD_HTML)
}

const DASHBOARD_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>AO Recorder Dashboard</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #0f1117; color: #e0e0e0; padding: 20px; }
  h1 { font-size: 1.4rem; margin-bottom: 4px; }
  .subtitle { color: #888; font-size: 0.85rem; margin-bottom: 20px; }
  .status-bar { display: flex; align-items: center; gap: 12px; margin-bottom: 20px; padding: 12px 16px; border-radius: 8px; background: #1a1d27; }
  .status-dot { width: 12px; height: 12px; border-radius: 50%; }
  .status-ok { background: #22c55e; }
  .status-degraded { background: #f59e0b; }
  .status-error { background: #ef4444; }
  .status-unknown { background: #666; }
  .status-text { font-weight: 600; text-transform: uppercase; font-size: 0.9rem; }
  .metrics { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 12px; margin-bottom: 20px; }
  .metric-card { background: #1a1d27; padding: 14px; border-radius: 8px; }
  .metric-label { font-size: 0.75rem; color: #888; text-transform: uppercase; letter-spacing: 0.05em; }
  .metric-value { font-size: 1.5rem; font-weight: 700; margin-top: 4px; }
  .gauge { margin-top: 8px; height: 6px; border-radius: 3px; background: #2a2d37; overflow: hidden; }
  .gauge-fill { height: 100%; border-radius: 3px; transition: width 0.3s; }
  .gauge-green { background: #22c55e; }
  .gauge-amber { background: #f59e0b; }
  .gauge-red { background: #ef4444; }
  table { width: 100%; border-collapse: collapse; background: #1a1d27; border-radius: 8px; overflow: hidden; }
  th { text-align: left; font-size: 0.75rem; color: #888; text-transform: uppercase; letter-spacing: 0.05em; padding: 10px 14px; border-bottom: 1px solid #2a2d37; }
  td { padding: 10px 14px; border-bottom: 1px solid #1e2130; font-size: 0.9rem; }
  tr:last-child td { border-bottom: none; }
  .chain-id { font-family: monospace; font-size: 0.8rem; color: #888; }
  .section-title { font-size: 1rem; font-weight: 600; margin: 20px 0 10px; }
  .refresh-info { font-size: 0.75rem; color: #555; margin-top: 16px; text-align: right; }
  .help-link { color: #60a5fa; text-decoration: none; font-size: 0.8rem; }
  .help-link:hover { text-decoration: underline; }
  .error-banner { background: #3b1111; border: 1px solid #ef4444; color: #fca5a5; padding: 12px 16px; border-radius: 8px; margin-bottom: 16px; display: none; }
  .help-section { background: #1a1d27; padding: 14px 18px; border-radius: 8px; margin-top: 12px; display: none; }
  .help-section h3 { font-size: 0.9rem; margin-bottom: 8px; }
  .help-section p, .help-section li { font-size: 0.85rem; color: #aaa; line-height: 1.6; }
  .help-section ul { padding-left: 20px; }
  .help-toggle { cursor: pointer; user-select: none; }
</style>
</head>
<body>
<h1>AO Recorder Dashboard</h1>
<p class="subtitle">
  <span id="version"></span> &middot; <span id="uptime"></span> uptime &middot;
  <a href="/health" class="help-link">JSON API</a> &middot;
  <span class="help-toggle help-link" onclick="toggleHelp()">Help</span>
</p>

<div id="error-banner" class="error-banner"></div>

<div id="help-panel" class="help-section">
  <h3>Dashboard Help</h3>
  <p>This dashboard auto-refreshes every 10 seconds from the <code>/health</code> endpoint. What to watch for:</p>
  <ul>
    <li><strong>Status dot</strong>: Green = all OK. Amber = degraded (low disk or no chains). Red = critical (very low disk).</li>
    <li><strong>RAM</strong>: Shows system memory usage. Gradual increase over days may indicate a memory leak — restart the recorder and report the issue.</li>
    <li><strong>Disk</strong>: Shows data directory filesystem usage. When disk approaches 90%, add storage or prune old blob data.</li>
    <li><strong>Chain table</strong>: "Last block age" shows how recently a block was recorded. If a chain shows many hours or days, the chain may be stalled — check that clients are submitting transactions, and check logs for errors.</li>
    <li><strong>UTXO count</strong>: The number of unspent transaction outputs. Rapid growth may indicate unreclaimed change outputs.</li>
  </ul>
  <p style="margin-top:8px">For detailed operations guidance, see the <strong>Sysop Guide</strong> included with your installation.</p>
</div>

<div class="status-bar">
  <div id="status-dot" class="status-dot status-unknown"></div>
  <span id="status-text" class="status-text">Connecting...</span>
</div>

<div class="metrics">
  <div class="metric-card">
    <div class="metric-label">RAM Used</div>
    <div class="metric-value" id="ram-used">—</div>
    <div class="gauge"><div class="gauge-fill" id="ram-gauge" style="width:0%"></div></div>
  </div>
  <div class="metric-card">
    <div class="metric-label">Disk Used</div>
    <div class="metric-value" id="disk-used">—</div>
    <div class="gauge"><div class="gauge-fill" id="disk-gauge" style="width:0%"></div></div>
  </div>
  <div class="metric-card">
    <div class="metric-label">Chains Hosted</div>
    <div class="metric-value" id="chain-count">—</div>
  </div>
  <div class="metric-card">
    <div class="metric-label">Total UTXOs</div>
    <div class="metric-value" id="utxo-total">—</div>
  </div>
</div>

<div class="section-title">Chains</div>
<table>
  <thead>
    <tr>
      <th>Symbol</th>
      <th>Block Height</th>
      <th>Last Block Age</th>
      <th>UTXOs</th>
      <th>DB Size</th>
      <th>Chain ID</th>
    </tr>
  </thead>
  <tbody id="chain-table">
    <tr><td colspan="6" style="text-align:center;color:#555">Loading...</td></tr>
  </tbody>
</table>

<div class="refresh-info">Auto-refresh: 10s &middot; Last update: <span id="last-update">—</span></div>

<script>
function formatBytes(b) {
  if (b < 1024) return b + ' B';
  if (b < 1048576) return (b / 1024).toFixed(1) + ' KB';
  if (b < 1073741824) return (b / 1048576).toFixed(1) + ' MB';
  return (b / 1073741824).toFixed(2) + ' GB';
}

function formatDuration(seconds) {
  if (seconds < 60) return Math.floor(seconds) + 's';
  if (seconds < 3600) return Math.floor(seconds / 60) + 'm ' + Math.floor(seconds % 60) + 's';
  if (seconds < 86400) return Math.floor(seconds / 3600) + 'h ' + Math.floor((seconds % 3600) / 60) + 'm';
  return Math.floor(seconds / 86400) + 'd ' + Math.floor((seconds % 86400) / 3600) + 'h';
}

function gaugeColor(percent) {
  if (percent >= 90) return 'gauge-red';
  if (percent >= 75) return 'gauge-amber';
  return 'gauge-green';
}

function toggleHelp() {
  var panel = document.getElementById('help-panel');
  panel.style.display = panel.style.display === 'block' ? 'none' : 'block';
}

async function refresh() {
  try {
    var resp = await fetch('/health');
    if (!resp.ok) throw new Error('HTTP ' + resp.status);
    var data = await resp.json();

    document.getElementById('error-banner').style.display = 'none';

    // Status
    var dot = document.getElementById('status-dot');
    dot.className = 'status-dot status-' + data.status;
    document.getElementById('status-text').textContent = data.status;

    // Version + uptime
    document.getElementById('version').textContent = 'v' + data.version;
    document.getElementById('uptime').textContent = formatDuration(data.uptime_seconds);

    // RAM
    var sys = data.system;
    var ramTotal = sys.ram_used_bytes + sys.ram_available_bytes;
    var ramPct = ramTotal > 0 ? (sys.ram_used_bytes / ramTotal * 100) : 0;
    document.getElementById('ram-used').textContent = formatBytes(sys.ram_used_bytes);
    var ramGauge = document.getElementById('ram-gauge');
    ramGauge.style.width = ramPct.toFixed(1) + '%';
    ramGauge.className = 'gauge-fill ' + gaugeColor(ramPct);

    // Disk
    var diskTotal = sys.disk_used_bytes + sys.disk_free_bytes;
    var diskPct = diskTotal > 0 ? (sys.disk_used_bytes / diskTotal * 100) : 0;
    document.getElementById('disk-used').textContent = formatBytes(sys.disk_used_bytes) + ' / ' + formatBytes(diskTotal);
    var diskGauge = document.getElementById('disk-gauge');
    diskGauge.style.width = diskPct.toFixed(1) + '%';
    diskGauge.className = 'gauge-fill ' + gaugeColor(diskPct);

    // Chains
    document.getElementById('chain-count').textContent = data.chains.length;
    var utxoTotal = data.chains.reduce(function(sum, c) { return sum + c.utxo_count; }, 0);
    document.getElementById('utxo-total').textContent = utxoTotal.toLocaleString();

    // Chain table
    var tbody = document.getElementById('chain-table');
    if (data.chains.length === 0) {
      tbody.innerHTML = '<tr><td colspan="6" style="text-align:center;color:#555">No chains hosted</td></tr>';
    } else {
      tbody.innerHTML = data.chains.map(function(c) {
        var age = c.last_block_age_seconds != null ? formatDuration(c.last_block_age_seconds) : '—';
        var ageStyle = '';
        if (c.last_block_age_seconds != null && c.last_block_age_seconds > 86400) ageStyle = 'color:#f59e0b';
        if (c.last_block_age_seconds != null && c.last_block_age_seconds > 604800) ageStyle = 'color:#ef4444';
        return '<tr>' +
          '<td><strong>' + c.symbol + '</strong></td>' +
          '<td>' + c.block_height.toLocaleString() + '</td>' +
          '<td style="' + ageStyle + '">' + age + '</td>' +
          '<td>' + c.utxo_count.toLocaleString() + '</td>' +
          '<td>' + formatBytes(c.db_size_bytes) + '</td>' +
          '<td class="chain-id">' + c.chain_id.substring(0, 16) + '…</td>' +
          '</tr>';
      }).join('');
    }

    document.getElementById('last-update').textContent = new Date().toLocaleTimeString();
  } catch (e) {
    document.getElementById('error-banner').textContent = 'Failed to fetch /health: ' + e.message + '. Is the recorder running?';
    document.getElementById('error-banner').style.display = 'block';
    document.getElementById('status-dot').className = 'status-dot status-unknown';
    document.getElementById('status-text').textContent = 'Unreachable';
  }
}

refresh();
setInterval(refresh, 10000);
</script>
</body>
</html>
"##;
