import { useEffect, useRef, useState, useCallback } from 'react';
import jsQR from 'jsqr';

interface QrScannerProps {
  onScan: (data: string) => void;
  onClose: () => void;
}

/** Camera-based QR code scanner using jsQR. */
export function QrScanner({ onScan, onClose }: QrScannerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [error, setError] = useState<string | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const rafRef = useRef<number>(0);

  const stopCamera = useCallback(() => {
    cancelAnimationFrame(rafRef.current);
    if (streamRef.current) {
      for (const track of streamRef.current.getTracks()) track.stop();
      streamRef.current = null;
    }
  }, []);

  useEffect(() => {
    let active = true;

    async function startCamera() {
      try {
        const stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: 'environment' },
        });
        if (!active) {
          for (const track of stream.getTracks()) track.stop();
          return;
        }
        streamRef.current = stream;
        if (videoRef.current) {
          videoRef.current.srcObject = stream;
          videoRef.current.play();
        }
      } catch (e) {
        setError(`Camera access denied: ${e}`);
      }
    }

    startCamera();
    return () => {
      active = false;
      stopCamera();
    };
  }, [stopCamera]);

  useEffect(() => {
    const video = videoRef.current;
    const canvas = canvasRef.current;
    if (!video || !canvas) return;

    const ctx = canvas.getContext('2d', { willReadFrequently: true });
    if (!ctx) return;

    function scan() {
      if (!video || !canvas || !ctx) return;
      if (video.readyState === video.HAVE_ENOUGH_DATA) {
        canvas.width = video.videoWidth;
        canvas.height = video.videoHeight;
        ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
        const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
        const code = jsQR(imageData.data, imageData.width, imageData.height);
        if (code) {
          stopCamera();
          onScan(code.data);
          return;
        }
      }
      rafRef.current = requestAnimationFrame(scan);
    }

    rafRef.current = requestAnimationFrame(scan);
    return () => cancelAnimationFrame(rafRef.current);
  }, [onScan, stopCamera]);

  return (
    <div style={{
      position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.8)',
      display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
      zIndex: 1000,
    }}>
      {error ? (
        <div style={{ color: '#fff', padding: 16, textAlign: 'center' }}>
          <div>{error}</div>
          <button onClick={onClose} style={{ marginTop: 12 }}>Close</button>
        </div>
      ) : (
        <>
          <video
            ref={videoRef}
            style={{ maxWidth: '90vw', maxHeight: '70vh', borderRadius: 8 }}
            playsInline
            muted
          />
          <canvas ref={canvasRef} style={{ display: 'none' }} />
          <div style={{ color: '#fff', marginTop: 12, fontSize: 14 }}>
            Point camera at a QR code
          </div>
          <button onClick={() => { stopCamera(); onClose(); }} style={{ marginTop: 8 }}>
            Cancel
          </button>
        </>
      )}
    </div>
  );
}
