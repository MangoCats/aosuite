# Assign Onward Installers

Build scripts for Debian (.deb) and Windows (.msi) packages.

## What Gets Installed

| Binary | Description | Default Port |
|--------|-------------|-------------|
| `ao` | CLI tool for chain operations | — |
| `ao-recorder` | Chain recording server | 3000 |
| `ao-validator` | Chain validator daemon | 4000 |
| `ao-exchange` | Exchange/market-making agent | 3100 |

## Debian / Ubuntu / Raspberry Pi OS

**Prerequisites**: `cargo`, `dpkg-deb`, `strip` (binutils)

```bash
cd installers/debian
chmod +x build.sh
./build.sh
```

Output: `out/ao-recorder_0.1.0_amd64.deb`, etc.

**Cross-compile for Raspberry Pi**:
```bash
./build.sh --target aarch64-unknown-linux-gnu
```

**Install**:
```bash
sudo dpkg -i out/ao-recorder_0.1.0_amd64.deb
```

Each daemon package:
- Creates a system user (`ao-recorder`, `ao-validator`, `ao-exchange`)
- Installs a systemd service (enabled but not started)
- Creates config in `/etc/<service>/` and data dir in `/var/lib/<service>/`
- On purge (`apt purge`), removes user, config, and data

## Windows

**Prerequisites**: `cargo`, [WiX Toolset v4+](https://wixtoolset.org/)

```powershell
# Install WiX v4
dotnet tool install --global wix

# Build
cd installers\windows
.\build.ps1
```

Output: `out\AssignOnward-0.1.0-x64.msi`

The MSI installs to `Program Files\AssignOnward\bin\`, adds the bin directory to the system PATH, and creates data directories under `%ProgramData%\AssignOnward\`.

**Install**:
```powershell
msiexec /i out\AssignOnward-0.1.0-x64.msi
```

## Post-Install

See [SysopGuide.md](../SysopGuide.md) for first-time setup, configuration, and operation.
