# dechat-rs

`dechat-rs` is a tool written in Rust, aimed at reducing keyboard chatter / debouncing.

Features:
- Customizable throttling timeouts for multiple keys or key ranges.
- Enhanced handling for repeated chatter scenarios.

Most de-chatter solutions simply block the next input if it matches the previous key code and immediately forget that
key if it was released. However, that approach doesn't always cut it. There are often cases of multiple subsequent 
bounces, like in "retr**t**ack", "plau**l**sible", "pottery**y**", etc.

This approach ensures that no key code repeats within a predefined time frame.

## Installation

Install [cargo-arch](https://github.com/wdv4758h/cargo-arch)

```bash 
cargo install cargo-arch
```

Run build:

```bash
cargo arch
```

Install the package:

```bash
sudo pacman -U dechat-rs-0.1.0-1-x86_64.pkg.tar.zst
```

## Usage

Root access is required.

### List devices

Devices are sorted by bus, vendor, product, version, name, and path for consistent order.

```bash
Usage: dechat-rs list [OPTIONS]

Options:
  -p, --path           Show the path to the input device
  -P, --physical-path  Show the physical path to the input device
  -n, --name           Show the name of the input device
  -i, --id             Show the bus, vendor, product, and version of the input device
  -k, --keys           Show all keys supported by the input device
  -a, --all            Enable all flags: (-Ppnik)
  -h, --help           Print help
  -V, --version        Print version
```

List all devices: 

```bash
sudo dechat-rs list
```
```
path=/dev/input/event9 physical_path=ALSA
path=/dev/input/event11 physical_path=ALSA
path=/dev/input/event10 physical_path=ALSA
path=/dev/input/event4 physical_path=usb-0000:04:00.3-3/input0
path=/dev/input/event5 physical_path=usb-0000:04:00.3-3/input2
path=/dev/input/event12 physical_path=?
```

List all devices with names and supported keys:

```bash
sudo dechat-rs list -Ppnik
# or 
sudo dechat-rs list -a
```

```
path=/dev/input/event3 physical_path=LNXVIDEO/video/input0 name=Video Bus bus=Host bus_id=0x19 vendor=0x0 product=0x6 version=0x0
        Keys: KEY_BRIGHTNESSDOWN=224, KEY_BRIGHTNESSUP=225, KEY_SWITCHVIDEOMODE=227, KEY_VIDEO_NEXT=241, KEY_VIDEO_PREV=242, KEY_BRIGHTNESS_CYCLE=243, KEY_BRIGHTNESS_AUTO=244, KEY_DISPLAY_OFF=245
path=/dev/input/event6 physical_path=asus-wireless/input0 name=Asus Wireless Radio Control bus=Host bus_id=0x19 vendor=0x1043 product=0x0 version=0x0
        Keys: KEY_RFKILL=247
```

### Start de-chattering

To de-chatter a specific device, use filters based on the device name. 
If multiple devices share a name, use these filters for distinction:
- `s:` - starts with (i.e. `de-chatter -t 0:1000:70 -n s:'Asus Keyboard' -P 'usb-0000:04:00.3-3/input2'`)
- `e:` or no prefix - equals 
- `c:` - contains

If filters aren't sufficient, use `--id` or `-i` to select by device index as shown in the de-chatter sub-command output:

```bash
sudo dechat-rs de-chatter -t 0:1000:70 -n s:'Asus'
[2024-01-30T17:18:43Z INFO  dechat_rs::execute] A device with index=0 after applying filters: Asus Keyboard (/dev/input/event4)
[2024-01-30T17:18:43Z INFO  dechat_rs::execute] A device with index=1 after applying filters: Asus Keyboard (/dev/input/event5)
[2024-01-30T17:18:43Z INFO  dechat_rs::execute] A device with index=2 after applying filters: Asus WMI hotkeys (/dev/input/event8)
[2024-01-30T17:18:43Z INFO  dechat_rs::execute] A device with index=3 after applying filters: Asus Wireless Radio Control (/dev/input/event6)
```

Pass throttling timeouts with key codes to filter. Format: `start_code_inclusive:end_code_inclusive:timeout`, e.g.:

```bash
sudo -E dechat-rs de-chatter -t 0:1000:70 -n 'Asus Keyboard' -P 'usb-0000:04:00.3-3/input2'
```

If the specified range is too large, it will be adjusted to the maximum supported range.

Periodically, the tool will display statistics about the number of throttled events:

``` 
[2024-01-30T16:58:33Z INFO  dechat_rs::key_filter] Throttled: KEY_BACKSPACE:14x13, KEY_E:18x2, KEY_Y:21x2, KEY_O:24x2, KEY_ENTER:28x10, KEY_LEFTCTRL:29x135, KEY_LEFTSHIFT:42x73, KEY_N:49x2, KEY_LEFTALT:56x6, KEY_UP:103x44, KEY_LEFT:105x10, KEY_RIGHT:106x19, KEY_DOWN:108x223
```
