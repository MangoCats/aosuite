import { useState, useEffect } from 'react';
import { useStore } from '../store/useStore.ts';
import { playChime, type ChimeStyle } from '../core/chime.ts';

const CHIME_OPTIONS: { value: ChimeStyle; label: string }[] = [
  { value: 'bell', label: 'Bell' },
  { value: 'cash', label: 'Cash Register' },
  { value: 'ding', label: 'Ding' },
  { value: 'none', label: 'Silent' },
];

const QUICK_MUTE_OPTIONS = [
  { label: '15 min', ms: 15 * 60_000 },
  { label: '1 hour', ms: 60 * 60_000 },
  { label: 'Until tomorrow', ms: 0 }, // computed at click time
];

function msUntilTomorrow(quietEnd: number): number {
  const now = new Date();
  const target = new Date(now);
  target.setDate(target.getDate() + 1);
  target.setHours(quietEnd, 0, 0, 0);
  return target.getTime() - now.getTime();
}

export function NotificationSettings() {
  const { notification, setNotification } = useStore();
  const [, forceUpdate] = useState(0);
  const isMuted = notification.quickMuteUntil !== null && Date.now() < notification.quickMuteUntil;

  // Auto-refresh UI when quick mute expires
  useEffect(() => {
    if (!notification.quickMuteUntil) return;
    const remaining = notification.quickMuteUntil - Date.now();
    if (remaining <= 0) return;
    const timer = setTimeout(() => forceUpdate(n => n + 1), remaining);
    return () => clearTimeout(timer);
  }, [notification.quickMuteUntil]);

  function handleQuickMute(ms: number) {
    const duration = ms === 0 ? msUntilTomorrow(notification.quietEnd) : ms;
    setNotification({ quickMuteUntil: Date.now() + duration });
  }

  function handleUnmute() {
    setNotification({ quickMuteUntil: null });
  }

  return (
    <div style={{ marginTop: 12, borderTop: '1px solid #eee', paddingTop: 12 }}>
      <div style={{ fontSize: 14, fontWeight: 500, marginBottom: 8 }}>Payment Notifications</div>

      {/* Master toggle */}
      <label style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8, fontSize: 13 }}>
        <input
          type="checkbox"
          checked={notification.enabled}
          onChange={e => setNotification({ enabled: e.target.checked })}
        />
        Enable sound notifications
      </label>

      {notification.enabled && (
        <>
          {/* Sound style */}
          <div style={{ marginBottom: 8 }}>
            <label style={{ fontSize: 13, fontWeight: 500 }}>Sound</label>
            <div style={{ display: 'flex', gap: 8, marginTop: 4 }}>
              <select
                value={notification.chimeStyle}
                onChange={e => setNotification({ chimeStyle: e.target.value as ChimeStyle })}
                style={{ padding: '4px 6px', fontSize: 13 }}
              >
                {CHIME_OPTIONS.map(o => (
                  <option key={o.value} value={o.value}>{o.label}</option>
                ))}
              </select>
              <button
                onClick={() => playChime(notification.chimeStyle, notification.volume)}
                style={{ fontSize: 12, padding: '4px 8px' }}
              >
                Test
              </button>
            </div>
          </div>

          {/* Volume */}
          <div style={{ marginBottom: 8 }}>
            <label style={{ fontSize: 13, fontWeight: 500 }}>
              Volume: {Math.round(notification.volume * 100)}%
            </label>
            <input
              type="range"
              min="0"
              max="100"
              value={Math.round(notification.volume * 100)}
              onChange={e => setNotification({ volume: Number(e.target.value) / 100 })}
              style={{ width: '100%', marginTop: 4 }}
            />
          </div>

          {/* Quiet hours */}
          <div style={{ marginBottom: 8 }}>
            <label style={{ fontSize: 13, fontWeight: 500 }}>Quiet Hours</label>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 4, fontSize: 13 }}>
              <select
                value={notification.quietStart}
                onChange={e => setNotification({ quietStart: Number(e.target.value) })}
                style={{ padding: '4px 6px' }}
              >
                {Array.from({ length: 24 }, (_, i) => (
                  <option key={i} value={i}>{String(i).padStart(2, '0')}:00</option>
                ))}
              </select>
              <span>to</span>
              <select
                value={notification.quietEnd}
                onChange={e => setNotification({ quietEnd: Number(e.target.value) })}
                style={{ padding: '4px 6px' }}
              >
                {Array.from({ length: 24 }, (_, i) => (
                  <option key={i} value={i}>{String(i).padStart(2, '0')}:00</option>
                ))}
              </select>
            </div>
          </div>

          {/* Quick mute */}
          <div>
            <label style={{ fontSize: 13, fontWeight: 500 }}>Quick Mute</label>
            {isMuted ? (
              <div style={{ marginTop: 4, fontSize: 13 }}>
                <span style={{ color: '#c00' }}>
                  Muted until {new Date(notification.quickMuteUntil!).toLocaleTimeString()}
                </span>
                <button
                  onClick={handleUnmute}
                  style={{ marginLeft: 8, fontSize: 12, padding: '2px 8px' }}
                >
                  Unmute
                </button>
              </div>
            ) : (
              <div style={{ display: 'flex', gap: 6, marginTop: 4 }}>
                {QUICK_MUTE_OPTIONS.map(o => (
                  <button
                    key={o.label}
                    onClick={() => handleQuickMute(o.ms)}
                    style={{ fontSize: 12, padding: '4px 8px' }}
                  >
                    {o.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}
