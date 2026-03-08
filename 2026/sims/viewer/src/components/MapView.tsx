import { useEffect, useRef } from 'react';
import L from 'leaflet';
import 'leaflet/dist/leaflet.css';
import { useStore } from '../store';
import type { AgentState, TransactionEvent } from '../api';

const ROLE_COLORS: Record<string, string> = {
  vendor: '#2b8a3e',
  exchange: '#e67700',
  consumer: '#1971c2',
  recorder: '#862e9c',
  validator: '#862e9c',
  attacker: '#e03131',
};

export function MapView() {
  const { agents, transactions, selectAgent, timeFilter, showHeatMap, toggleHeatMap, showCoverage, toggleCoverage, showAuditOverlay, toggleAuditOverlay } = useStore();
  const mapRef = useRef<L.Map | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const markersRef = useRef<Map<string, L.CircleMarker>>(new Map());
  const arcsRef = useRef<L.Polyline[]>([]);
  const heatRef = useRef<L.Circle[]>([]);
  const coverageRef = useRef<L.Circle[]>([]);
  const auditRef = useRef<L.Circle[]>([]);
  const tooltipLayerRef = useRef<L.LayerGroup | null>(null);

  // Initialize map once
  useEffect(() => {
    if (!containerRef.current || mapRef.current) return;

    // Find center from agents with valid positions
    const positioned = agents.filter((a) => a.lat !== 0 || a.lon !== 0);
    const center: [number, number] = positioned.length > 0
      ? [
          positioned.reduce((s, a) => s + a.lat, 0) / positioned.length,
          positioned.reduce((s, a) => s + a.lon, 0) / positioned.length,
        ]
      : [18.205, -63.05]; // Anguilla default

    const map = L.map(containerRef.current, {
      center,
      zoom: 15,
      zoomControl: true,
    });

    L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
      attribution: '&copy; OpenStreetMap contributors',
      maxZoom: 19,
    }).addTo(map);

    tooltipLayerRef.current = L.layerGroup().addTo(map);
    mapRef.current = map;

    return () => {
      map.remove();
      mapRef.current = null;
      markersRef.current.clear();
      arcsRef.current = [];
      tooltipLayerRef.current = null;
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Update markers when agents change
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;

    const existingNames = new Set(markersRef.current.keys());
    const currentNames = new Set(agents.map((a) => a.name));

    // Remove markers for agents no longer present
    for (const name of existingNames) {
      if (!currentNames.has(name)) {
        markersRef.current.get(name)?.remove();
        markersRef.current.delete(name);
      }
    }

    // Add/update markers
    for (const agent of agents) {
      if (agent.lat === 0 && agent.lon === 0) continue;

      const existing = markersRef.current.get(agent.name);
      if (existing) {
        existing.setLatLng([agent.lat, agent.lon]);
        existing.setStyle({ fillColor: ROLE_COLORS[agent.role] || '#868e96' });
        existing.unbindTooltip();
        existing.bindTooltip(buildTooltip(agent), { direction: 'top', offset: [0, -8] });
      } else {
        const marker = L.circleMarker([agent.lat, agent.lon], {
          radius: agent.role === 'vendor' ? 10 : agent.role === 'exchange' ? 8 : 6,
          fillColor: ROLE_COLORS[agent.role] || '#868e96',
          color: '#fff',
          weight: 2,
          fillOpacity: 0.9,
        });
        marker.bindTooltip(buildTooltip(agent), { direction: 'top', offset: [0, -8] });
        marker.on('click', () => selectAgent(agent.name));
        marker.addTo(map);
        markersRef.current.set(agent.name, marker);
      }
    }
  }, [agents, selectAgent]);

  // Draw transaction arcs for recent transactions
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;

    // Remove old arcs
    for (const arc of arcsRef.current) {
      arc.remove();
    }
    arcsRef.current = [];

    // Draw arcs for last 20 transactions (filtered by time if paused)
    const visible = timeFilter !== null
      ? transactions.filter((t) => t.timestamp_ms <= timeFilter)
      : transactions;
    const recent = visible.slice(-20);
    const agentMap = new Map(agents.map((a) => [a.name, a]));

    for (const tx of recent) {
      const from = agentMap.get(tx.from_agent);
      const to = agentMap.get(tx.to_agent);
      if (!from || !to) continue;
      if (from.lat === 0 && from.lon === 0) continue;
      if (to.lat === 0 && to.lon === 0) continue;

      const age = Date.now() - tx.timestamp_ms;
      const opacity = Math.max(0.1, 1 - age / 60000); // fade over 60s
      const isCaa = tx.description.includes('CAA');

      const arc = L.polyline(
        [[from.lat, from.lon], [to.lat, to.lon]],
        {
          color: isCaa ? '#7048e8' : (ROLE_COLORS[from.role] || '#868e96'),
          weight: isCaa ? 3 : 2,
          opacity,
          dashArray: isCaa ? undefined : '6 4',
        },
      );
      arc.bindPopup(formatTxPopup(tx));
      arc.addTo(map);
      arcsRef.current.push(arc);
    }
  }, [agents, transactions, timeFilter]);

  // Heat map overlay: semi-transparent circles at transaction midpoints (debounced)
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;

    if (!showHeatMap) {
      for (const c of heatRef.current) c.remove();
      heatRef.current = [];
      return;
    }

    const timer = setTimeout(() => {
      for (const c of heatRef.current) c.remove();
      heatRef.current = [];

      const agentMap = new Map(agents.map((a) => [a.name, a]));
      const visible = timeFilter !== null
        ? transactions.filter((t) => t.timestamp_ms <= timeFilter)
        : transactions;
      const now = Date.now();

      for (const tx of visible.slice(-100)) {
        const from = agentMap.get(tx.from_agent);
        const to = agentMap.get(tx.to_agent);
        if (!from || !to) continue;
        if ((from.lat === 0 && from.lon === 0) || (to.lat === 0 && to.lon === 0)) continue;

        const midLat = (from.lat + to.lat) / 2;
        const midLon = (from.lon + to.lon) / 2;
        const age = now - tx.timestamp_ms;
        const opacity = Math.max(0.05, 0.4 * (1 - age / 300000)); // fade over 5 min

        const circle = L.circle([midLat, midLon], {
          radius: 80,
          color: 'transparent',
          fillColor: '#e03131',
          fillOpacity: opacity,
        }).addTo(map);
        heatRef.current.push(circle);
      }
    }, 300);

    return () => clearTimeout(timer);
  }, [agents, transactions, timeFilter, showHeatMap]);

  // Coverage zones: circles around vendors
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;

    for (const c of coverageRef.current) c.remove();
    coverageRef.current = [];

    if (!showCoverage) return;

    for (const agent of agents) {
      if (agent.coverage_radius == null) continue;
      if (agent.lat === 0 && agent.lon === 0) continue;

      const circle = L.circle([agent.lat, agent.lon], {
        radius: agent.coverage_radius,
        color: ROLE_COLORS[agent.role] || '#868e96',
        weight: 1,
        fillColor: ROLE_COLORS[agent.role] || '#868e96',
        fillOpacity: 0.1,
        dashArray: '4 4',
      }).addTo(map);
      coverageRef.current.push(circle);
    }
  }, [agents, showCoverage]);

  // Audit overlay: halos around vendors showing chain validation status
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;

    if (!showAuditOverlay) {
      for (const c of auditRef.current) c.remove();
      auditRef.current = [];
      return;
    }

    const timer = setTimeout(() => {
      for (const c of auditRef.current) c.remove();
      auditRef.current = [];

      // Collect validator chain statuses
      const chainStatus = new Map<string, string>(); // chain_id -> "ok" | "alert"
      for (const a of agents) {
        if (!a.validator_status) continue;
        for (const mc of a.validator_status.monitored_chains) {
          const existing = chainStatus.get(mc.chain_id);
          // alert overrides ok
          if (!existing || mc.status === 'alert') {
            chainStatus.set(mc.chain_id, mc.status);
          }
        }
      }

      // Draw halos around vendors
      for (const agent of agents) {
        if (agent.role !== 'vendor') continue;
        if (agent.lat === 0 && agent.lon === 0) continue;
        if (agent.chains.length === 0) continue;

        const chainId = agent.chains[0].chain_id;
        const status = chainStatus.get(chainId);
        const color = status === 'ok' ? '#2b8a3e' : status === 'alert' ? '#e03131' : '#868e96';

        const circle = L.circle([agent.lat, agent.lon], {
          radius: 120,
          color,
          weight: 3,
          fillColor: color,
          fillOpacity: 0.08,
          dashArray: status === 'ok' ? undefined : '8 4',
        }).addTo(map);
        auditRef.current.push(circle);
      }
    }, 300);

    return () => clearTimeout(timer);
  }, [agents, showAuditOverlay]);

  return (
    <div style={{ position: 'relative' }}>
      <div
        ref={containerRef}
        style={{ width: '100%', height: 500, borderRadius: 8, border: '1px solid #dee2e6' }}
      />
      <div style={{
        position: 'absolute', top: 10, right: 10, zIndex: 1000,
        display: 'flex', gap: 4, flexDirection: 'column',
      }}>
        <button
          onClick={toggleHeatMap}
          style={{
            ...overlayBtnStyle,
            background: showHeatMap ? '#e03131' : '#fff',
            color: showHeatMap ? '#fff' : '#333',
          }}
        >
          Heat Map
        </button>
        <button
          onClick={toggleCoverage}
          style={{
            ...overlayBtnStyle,
            background: showCoverage ? '#2b8a3e' : '#fff',
            color: showCoverage ? '#fff' : '#333',
          }}
        >
          Coverage
        </button>
        <button
          onClick={toggleAuditOverlay}
          style={{
            ...overlayBtnStyle,
            background: showAuditOverlay ? '#862e9c' : '#fff',
            color: showAuditOverlay ? '#fff' : '#333',
          }}
        >
          Audit
        </button>
      </div>
    </div>
  );
}

function buildTooltip(agent: AgentState): string {
  const utxos = agent.chains.reduce((s, c) => s + c.unspent_utxos, 0);
  const chains = agent.chains.map((c) => c.symbol).join(', ');
  return `<strong>${agent.name}</strong><br/>` +
    `${agent.role} — ${agent.status}<br/>` +
    `${agent.transactions} txns, ${utxos} UTXOs<br/>` +
    (chains ? `Chains: ${chains}` : '');
}

function formatTxPopup(tx: TransactionEvent): string {
  const time = new Date(tx.timestamp_ms).toLocaleTimeString();
  return `<strong>${tx.symbol}</strong> ${tx.from_agent} → ${tx.to_agent}<br/>` +
    `${tx.description}<br/>` +
    `Block ${tx.block_height} at ${time}`;
}

const overlayBtnStyle: React.CSSProperties = {
  padding: '4px 10px', fontSize: 12, fontWeight: 600,
  border: '1px solid #dee2e6', borderRadius: 4, cursor: 'pointer',
  boxShadow: '0 1px 3px rgba(0,0,0,0.15)',
};
