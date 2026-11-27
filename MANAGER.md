# Dechat-rs Manager

A helper script to easily manage `dechat-rs` instances as systemd services on Linux. 

This tool simplifies the process of identifying your devices, testing debounce settings, and making the fix persistent across reboots without manually editing systemd unit files.

## Prerequisites

- **Root Access**: The script requires `sudo` privileges to interact with systemd and input devices.
- **dechat-rs**: You must have `dechat-rs` installed and available in your system PATH (e.g., `/usr/bin/dechat-rs` or `/usr/local/bin/dechat-rs`).

## Installation

1. Download the script:
   ```bash
   wget https://raw.githubusercontent.com/VisualBoy/dechat-rs/dechat-manager/dechat-manager.sh
   ```

2. Make it executable:
   ```bash
   chmod +x dechat-manager.sh
   ```

3. Run it:
   ```bash
   sudo ./dechat-manager.sh
   ```

## Usage Guide

When you launch the manager, you will be presented with a main menu:

### 1. Create new device configuration
Use this option to set up a debounce filter for a new keyboard or mouse.

1. **Device Identification**: The script lists all available input devices. Enter a unique string that matches the name of your target device (e.g., "Logitech G915"). The filter uses a "starts with" logic.
2. **Debounce Configuration**: Set the debounce threshold in milliseconds (default is **45ms**). This is the window in which duplicate keypresses will be ignored.
3. **Restart Policy**: Choose how the service handles failures (Default: `always`).

The script will automatically generate a valid systemd service file (e.g., `dechat-logitech-g915.service`), enable it, and start it immediately.

### 2. Manage/Remove existing configurations
View and control all `dechat-rs` services created by this manager.

- **Status Indicator**: See at a glance which services are `[active]` (green) or `[inactive]` (red).
- **Actions**:
  - **Start/Stop/Restart**: Control the service instantly.
  - **Disable & Delete**: Permanently remove the configuration file and disable the service from starting at boot.

## Troubleshooting

If a service fails to start:
1. Use the **Manage** menu to check the status.
2. If it is red (inactive), check the system logs manually:
   ```bash
   journalctl -u dechat-your-device-name.service -f
   ```
3. Ensure that the device name string you provided is correct and unique enough to not conflict with other devices (like your mouse).

## License

This script is distributed under the MIT License.
