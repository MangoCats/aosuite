import { useEffect, useRef } from 'react';
import QRCode from 'qrcode';

interface QrCodeProps {
  value: string;
  size?: number;
}

/** Render a QR code to a canvas element. */
export function QrCode({ value, size = 200 }: QrCodeProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (!canvasRef.current || !value) return;
    QRCode.toCanvas(canvasRef.current, value, {
      width: size,
      margin: 2,
      errorCorrectionLevel: 'M',
    }).catch(() => {
      // silently ignore rendering errors
    });
  }, [value, size]);

  return <canvas ref={canvasRef} />;
}
