import { useEffect, useRef } from 'react';
import L from 'leaflet';
import 'leaflet/dist/leaflet.css';

export interface VendorPin {
  symbol: string;
  name: string;
  lat: number;
  lon: number;
  chainId?: string;
}

interface VendorMapProps {
  vendors: VendorPin[];
  center?: [number, number];
  zoom?: number;
  height?: number;
}

/** Leaflet/OpenStreetMap display of vendor locations. */
export function VendorMap({ vendors, center, zoom = 14, height = 300 }: VendorMapProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const mapRef = useRef<L.Map | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    // Default center: first vendor or (0,0)
    const mapCenter = center ?? (
      vendors.length > 0 ? [vendors[0].lat, vendors[0].lon] : [0, 0]
    ) as [number, number];

    const map = L.map(containerRef.current).setView(mapCenter, zoom);
    mapRef.current = map;

    L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
      attribution: '&copy; OpenStreetMap contributors',
      maxZoom: 19,
    }).addTo(map);

    // Simple circle markers (avoids needing Leaflet's default icon images)
    for (const v of vendors) {
      L.circleMarker([v.lat, v.lon], {
        radius: 8,
        fillColor: '#2563eb',
        color: '#fff',
        weight: 2,
        fillOpacity: 0.9,
      })
        .addTo(map)
        .bindPopup(() => {
          const el = document.createElement('div');
          const sym = document.createElement('b');
          sym.textContent = v.symbol;
          el.appendChild(sym);
          el.appendChild(document.createElement('br'));
          el.appendChild(document.createTextNode(v.name));
          return el;
        });
    }

    return () => {
      map.remove();
      mapRef.current = null;
    };
  }, [vendors, center, zoom]);

  return <div ref={containerRef} style={{ height, width: '100%', borderRadius: 4 }} />;
}
