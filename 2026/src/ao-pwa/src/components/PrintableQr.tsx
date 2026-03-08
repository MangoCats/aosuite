import { useState, useEffect, useRef } from 'react';
import QRCode from 'qrcode';

interface PrintableQrProps {
  chainUrl: string;
  symbol: string;
  businessName: string;
}

/** Printable QR signage for vendor countertop/wall display.
 *  Renders a high-DPI QR code with chain symbol, business name, and "Scan to pay" label.
 *  Supports browser print dialog (for print/PDF) and PNG download. */
export function PrintableQr({ chainUrl, symbol, businessName }: PrintableQrProps) {
  const [svgHtml, setSvgHtml] = useState('');
  const printRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    QRCode.toString(chainUrl, {
      type: 'svg',
      margin: 2,
      errorCorrectionLevel: 'M',
      width: 400,
    }).then(svg => setSvgHtml(svg)).catch(() => {});
  }, [chainUrl]);

  function handlePrint() {
    const el = printRef.current;
    if (!el) return;
    const printWindow = window.open('', '_blank');
    if (!printWindow) return;
    printWindow.document.write(`<!DOCTYPE html>
<html><head><title>QR Signage — ${symbol}</title>
<style>
  body { margin: 0; display: flex; justify-content: center; align-items: center; min-height: 100vh; font-family: sans-serif; }
  .signage { text-align: center; padding: 24px; }
  .signage svg { width: 80mm; height: 80mm; }
  .symbol { font-size: 48pt; font-weight: bold; margin-top: 12px; }
  .name { font-size: 18pt; margin-top: 8px; color: #333; }
  .cta { font-size: 14pt; margin-top: 12px; color: #666; }
  @media print { body { margin: 0; } }
</style></head>
<body><div class="signage">
  ${svgHtml}
  <div class="symbol">${escapeHtml(symbol)}</div>
  ${businessName ? `<div class="name">${escapeHtml(businessName)}</div>` : ''}
  <div class="cta">Scan to pay</div>
</div></body></html>`);
    printWindow.document.close();
    // Wait for content to render before printing; close window after
    printWindow.onload = () => {
      printWindow.focus();
      printWindow.print();
      printWindow.onafterprint = () => printWindow.close();
    };
  }

  async function handleDownloadPng() {
    const canvas = document.createElement('canvas');
    const scale = 4; // high-DPI: 400 × scale = 1600px
    const qrSize = 400 * scale;
    const padding = 40 * scale;
    const textHeight = 120 * scale;
    canvas.width = qrSize + padding * 2;
    canvas.height = qrSize + padding * 2 + textHeight;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.fillStyle = '#fff';
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    // Draw QR code from SVG
    const qrDataUrl = await QRCode.toDataURL(chainUrl, {
      width: qrSize,
      margin: 2,
      errorCorrectionLevel: 'M',
    });
    const img = new Image();
    await new Promise<void>((resolve, reject) => {
      img.onload = () => resolve();
      img.onerror = reject;
      img.src = qrDataUrl;
    });
    ctx.drawImage(img, padding, padding, qrSize, qrSize);

    // Draw text
    const centerX = canvas.width / 2;
    let textY = padding + qrSize + 30 * scale;

    ctx.fillStyle = '#000';
    ctx.font = `bold ${36 * scale}px sans-serif`;
    ctx.textAlign = 'center';
    ctx.fillText(symbol, centerX, textY);
    textY += 40 * scale;

    if (businessName) {
      ctx.fillStyle = '#333';
      ctx.font = `${16 * scale}px sans-serif`;
      ctx.fillText(businessName, centerX, textY);
      textY += 24 * scale;
    }

    ctx.fillStyle = '#666';
    ctx.font = `${14 * scale}px sans-serif`;
    ctx.fillText('Scan to pay', centerX, textY);

    // Trigger download
    const link = document.createElement('a');
    link.download = `qr-${symbol.toLowerCase().replace(/[^a-z0-9-]/g, '_')}.png`;
    link.href = canvas.toDataURL('image/png');
    link.click();
  }

  return (
    <div>
      {/* Preview */}
      <div ref={printRef} style={{
        textAlign: 'center', padding: 16,
        border: '1px solid #ddd', borderRadius: 4, background: '#fff',
        marginBottom: 8,
      }}>
        <div dangerouslySetInnerHTML={{ __html: svgHtml }}
          style={{ width: 200, height: 200, margin: '0 auto' }} />
        <div style={{ fontSize: 24, fontWeight: 'bold', marginTop: 8 }}>{symbol}</div>
        {businessName && (
          <div style={{ fontSize: 14, color: '#333', marginTop: 4 }}>{businessName}</div>
        )}
        <div style={{ fontSize: 12, color: '#666', marginTop: 6 }}>Scan to pay</div>
      </div>

      {/* Actions */}
      <div style={{ display: 'flex', gap: 8 }}>
        <button onClick={handlePrint} style={{ flex: 1, fontSize: 13 }}>
          Print / PDF
        </button>
        <button onClick={handleDownloadPng} style={{ flex: 1, fontSize: 13 }}>
          Download PNG
        </button>
      </div>
    </div>
  );
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
