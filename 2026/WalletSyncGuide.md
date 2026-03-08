# Wallet Sync Guide

How to access your Assign Onward coins from multiple devices — your phone, your desktop, or a shared family wallet.

---

## How Your Wallet Works

Your wallet is a collection of **coin-keys** — Ed25519 private keys that control shares on AO chains. Each time you receive coins (from a purchase, transfer, or change), a new key is created on the device that made the transaction. That key lives only on that device until you sync it.

**Key facts:**
- Each coin-key is used once to receive and once to spend. Every transaction creates fresh keys.
- Your keys are encrypted with your wallet passphrase and never leave your device unencrypted.
- The recorder (the server hosting your chain) tracks which keys are spent or unspent — your device checks with the recorder to show you an accurate balance.

---

## Single Device (No Sync Needed)

If you only use one device, no setup is required. Your wallet works out of the box:

1. **Generate** a wallet (or import an existing seed).
2. **Set your passphrase** to encrypt your keys.
3. **Transact** — new keys are created and stored automatically.
4. **Balances update** each time you connect to the recorder.

**Important:** If you lose this device and have no backup, your coins are inaccessible. They will eventually expire and return to the community (typically after one year). Back up your wallet regularly.

---

## Syncing Between Your Own Devices (QR Transfer)

Use QR transfer to copy coin-keys between devices you own — your phone and your laptop, for example.

### Initial Setup

1. On your **primary device** (the one with coins), go to **Wallet > Export**.
2. Choose **Full Wallet Export**. This generates a QR code containing all your keys (encrypted with your wallet passphrase).
3. On your **second device**, go to **Wallet > Import > Scan QR**.
4. Scan the QR code from the primary device.
5. Enter your **wallet passphrase** when prompted.
6. The second device imports all keys and validates them against the recorder.

### After Each Transaction

When you buy something or receive coins on one device, that device creates new keys. To sync:

1. The device that transacted shows a **"Sync: N keys"** badge in the header.
2. Tap the badge to open the sync screen — a QR code appears.
3. On your other device, scan the QR code.
4. Enter your passphrase. The new keys are imported.

**Tip:** You don't need to sync immediately. Your other device will still show its last-known balance. When it connects to the recorder, it will detect any spent keys and update accordingly. You just won't be able to *spend* the new keys from the other device until you sync them.

### Spend Detection

If you spend coins from your phone, your desktop doesn't have the new change keys yet. When the desktop connects to the recorder:

- It discovers the old key was spent.
- It shows: **"Spent by [Your Phone]"** (if the devices are paired) or **"Spent by unknown device"** (if not yet synced).
- Once you sync the new change keys via QR, the desktop shows the correct balance.

### File-Based Transfer (Desktop to Desktop)

If neither desktop has a camera:

1. On the source desktop: **Wallet > Export > Save as File**. This saves an encrypted JSON file.
2. Transfer the file (USB drive, AirDrop, secure file share — the file is encrypted).
3. On the target desktop: **Wallet > Import > Load File**.
4. Enter your passphrase.

---

## Shared Wallet (Paired Devices)

For households or business partners who share a coin wallet across separate devices (e.g., husband's phone and wife's desktop), use **device pairing** for automatic real-time sync.

### How It Works

Paired devices communicate through an encrypted relay server. When one device creates or spends keys, the others are notified within seconds. The relay never sees your actual keys — everything is end-to-end encrypted.

### Pairing Two Devices

1. On **Device A**, go to **Settings > Paired Devices > Add Device**.
2. Device A displays a QR code.
3. On **Device B**, go to **Settings > Paired Devices > Scan Pair Code**.
4. Scan the QR code. Both devices briefly show a confirmation screen.
5. Done. The devices are now paired and will sync automatically.

### What Gets Synced Automatically

- **New keys** — when either device transacts, the new keys are pushed to the other immediately.
- **Spent keys** — when either device spends, the other is notified with the device name that spent.
- **Heartbeats** — devices ping each other every 5 minutes so you can see online/offline status.

### Managing Paired Devices

In **Settings > Paired Devices**, you can see:

- Each paired device's name and online/offline status.
- When each device was last seen.
- An **Unpair** button to remove a device from the group.

### More Than Two Devices

You can pair additional devices (e.g., phone + tablet + desktop). Each new device scans a QR code from any already-paired device. All paired devices in the group stay in sync.

### Relay Server

Paired devices communicate through a relay server. The relay is configured in **Settings > Sync > Relay URL**. By default, the app uses a public relay. You can run your own relay server for full control — it's a lightweight WebSocket forwarder.

The relay **cannot read your keys**. All messages are encrypted before leaving your device using a shared key established during pairing. The relay just forwards encrypted blobs.

---

## If a Device Is Lost or Stolen

1. **Don't panic.** Your keys on the lost device are encrypted with your wallet passphrase.
2. From any remaining paired device, go to **Settings > Paired Devices** and **Unpair** the lost device. This prevents it from receiving future sync messages.
3. As a precaution, **transfer your coins to fresh keys** — go to **Wallet > Refresh All Keys**. This self-assigns all your coins to newly generated keys, invalidating any keys the lost device holds.
4. The refresh costs a small recording fee per chain but ensures the lost device's keys are worthless.

**If you only have one device and it's lost:**
- If you have a file backup, import it on a new device and refresh keys immediately.
- If you have no backup, the coins will eventually expire (typically one year) and return to the community. There is no recovery mechanism — this is the trade-off of self-sovereign custody.

---

## Backup Recommendations

| Situation | Recommendation |
|-----------|---------------|
| Single user, one device | QR or file export after each transaction session. Store backup securely. |
| Single user, two devices | QR sync keeps both devices current. Either device is a live backup of the other. |
| Shared wallet (paired) | Paired devices are live backups. Also keep an offline file export updated monthly. |
| High-value holdings | File export to encrypted USB drive, stored physically separate from devices. |

**Your wallet passphrase is critical.** If you forget it, encrypted backups and synced keys cannot be decrypted. Choose a strong passphrase you won't forget, or store it securely (password manager, written down in a safe).

---

## Troubleshooting

**"Sync: N keys" badge won't go away**
You have unsynced keys. Open the sync screen and scan the QR from your other device, or pair devices for automatic sync.

**"Spent by unknown device" warning**
A key was spent and the spending device isn't in your paired devices list. If you have another device that could have spent it, sync or pair it. If not, this may indicate compromise — refresh all keys immediately.

**Paired device shows "Offline"**
The device hasn't sent a heartbeat in 10 minutes. It may be powered off, have no internet, or the relay may be unreachable. Sync will resume when both devices are online.

**QR code too complex to scan**
Large wallets produce multi-frame animated QR codes. Hold your scanning device steady and wait for all frames to be captured. If scanning fails, use file-based transfer instead.

**Balance differs between devices**
Devices may show different balances if one has unsynced keys. Sync or pair the devices, then both will connect to the recorder and show the same validated balance.
