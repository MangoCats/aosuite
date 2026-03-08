import { useStore } from '../store';

const ROLE_COLORS: Record<string, string> = {
  vendor: '#2b8a3e',
  exchange: '#e67700',
  consumer: '#1971c2',
  recorder: '#862e9c',
  validator: '#862e9c',
  attacker: '#e03131',
};

const ROLE_LABELS: Record<string, string> = {
  vendor: 'Vendor — sells goods, runs their own blockchain',
  exchange: 'Exchange — bridges between chains, earns a spread',
  consumer: 'Consumer — buys goods through exchange agents',
  recorder: 'Recorder — hosts blockchain data',
  validator: 'Validator — verifies chain integrity',
  attacker: 'Attacker — tests system security (should always fail)',
};

export function WelcomeOverlay() {
  const { scenarioMeta, showWelcome, setShowWelcome } = useStore();

  if (!showWelcome || !scenarioMeta) return null;

  // Don't show if no description (minimal scenarios without metadata)
  if (!scenarioMeta.description && !scenarioMeta.title) return null;

  const title = scenarioMeta.title || scenarioMeta.name;

  return (
    <div style={backdropStyle} onClick={() => setShowWelcome(false)}>
      <div style={panelStyle} onClick={(e) => e.stopPropagation()}>
        <h2 style={{ margin: '0 0 8px', fontSize: 20, fontWeight: 700 }}>{title}</h2>

        {scenarioMeta.description && (
          <p style={{ margin: '0 0 16px', fontSize: 14, color: '#495057', lineHeight: 1.5 }}>
            {scenarioMeta.description}
          </p>
        )}

        {scenarioMeta.what_to_watch.length > 0 && (
          <div style={{ marginBottom: 16 }}>
            <h3 style={sectionTitle}>What to watch</h3>
            <ul style={{ margin: 0, paddingLeft: 20, fontSize: 13, color: '#495057', lineHeight: 1.7 }}>
              {scenarioMeta.what_to_watch.map((item, i) => (
                <li key={i}>{item}</li>
              ))}
            </ul>
          </div>
        )}

        <div style={{ marginBottom: 16 }}>
          <h3 style={sectionTitle}>Cast</h3>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            {scenarioMeta.agents.map((a) => (
              <div key={a.name} style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
                <span style={{
                  display: 'inline-block', width: 10, height: 10, borderRadius: '50%',
                  background: ROLE_COLORS[a.role] || '#868e96', flexShrink: 0,
                }} />
                <strong style={{ minWidth: 80 }}>{a.name}</strong>
                <span style={{
                  padding: '1px 6px', borderRadius: 3, fontSize: 11, fontWeight: 600,
                  background: ROLE_COLORS[a.role] || '#868e96', color: '#fff',
                }}>
                  {a.role}
                </span>
                {a.blurb && <span style={{ color: '#666' }}>{a.blurb}</span>}
              </div>
            ))}
          </div>
        </div>

        <div style={{ marginBottom: 20 }}>
          <h3 style={sectionTitle}>Legend</h3>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 12, fontSize: 12, color: '#495057' }}>
            {Object.entries(ROLE_LABELS).map(([role, label]) => (
              <div key={role} style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                <span style={{
                  display: 'inline-block', width: 10, height: 10, borderRadius: '50%',
                  background: ROLE_COLORS[role],
                }} />
                <span><strong>{role}</strong> — {label.split(' — ')[1]}</span>
              </div>
            ))}
          </div>
        </div>

        <button onClick={() => setShowWelcome(false)} style={dismissBtn}>
          Got it — show me the map
        </button>
      </div>
    </div>
  );
}

const backdropStyle: React.CSSProperties = {
  position: 'fixed', inset: 0, zIndex: 10000,
  background: 'rgba(0, 0, 0, 0.5)',
  display: 'flex', alignItems: 'center', justifyContent: 'center',
  padding: 20,
};

const panelStyle: React.CSSProperties = {
  background: '#fff', borderRadius: 12, padding: '24px 28px',
  maxWidth: 600, maxHeight: '85vh', overflowY: 'auto',
  boxShadow: '0 8px 32px rgba(0,0,0,0.2)',
};

const sectionTitle: React.CSSProperties = {
  fontSize: 12, fontWeight: 700, textTransform: 'uppercase', color: '#868e96',
  margin: '0 0 8px', letterSpacing: '0.5px',
};

const dismissBtn: React.CSSProperties = {
  width: '100%', padding: '10px 20px',
  background: '#228be6', color: '#fff', border: 'none', borderRadius: 6,
  fontSize: 14, fontWeight: 600, cursor: 'pointer',
};
