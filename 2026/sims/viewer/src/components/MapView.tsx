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
};

export function MapView() {
  const { agents, transactions, selectAgent, timeFilter } = useStore();
  const mapRef = useRef<L.Map | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const markersRef = useRef<Map<string, L.CircleMarker>>(new Map());
  const arcsRef = useRef<L.Polyline[]>([]);
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

      const arc = L.polyline(
        [[from.lat, from.lon], [to.lat, to.lon]],
        {
          color: ROLE_COLORS[from.role] || '#868e96',
          weight: 2,
          opacity,
          dashArray: '6 4',
        },
      );
      arc.bindPopup(formatTxPopup(tx));
      arc.addTo(map);
      arcsRef.current.push(arc);
    }
  }, [agents, transactions, timeFilter]);

  return (
    <div
      ref={containerRef}
      style={{ width: '100%', height: 500, borderRadius: 8, border: '1px solid #dee2e6' }}
    />
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
