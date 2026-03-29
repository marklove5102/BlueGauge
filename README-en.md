# BlueGauge
A lightweight tray tool for easily viewing the battery level of your Bluetooth devices.

<p align="center">
	<a href="https://github.com/iKineticate/BlueGauge/releases/latest"><img alt="GitHub Release" src="https://img.shields.io/github/v/release/iKineticate/BlueGauge?"></a>
	<a href="https://github.com/iKineticate/BlueGauge/releases/latest"><img alt="Github Downloads" src="https://img.shields.io/github/downloads/iKineticate/BlueGauge/total?logo=github"></a>
    <img alt="Language" src="https://img.shields.io/badge/build-Rust-yellow?logo=rust">
    <a href="https://github.com/iKineticate/BlueGauge#"><img alt="GitHub License" src="https://img.shields.io/github/license/iKineticate/BlueGauge"></a>
</p>

<div align="center">
  <img src="screenshots/app.png" alt="App Screenshot" style="width: 100%; max-width: 100%; height: auto; display: block;" />
</div>

<h3 align="center"> <a href='./README.md'>简体中文</a> | English</h3>

## Function

1. Setting: Bluetooth battery level as tray icon  

<details>
<summary>Use number icon (default)</summary>

1. check the device that needs to display the battery    
2. set font: open tray menu -- `Settings` -- `Open Config`   
`font_name` = `"System Font Nmae, e.g. Microsoft YaHei UI"`  
`font_color` = `"Hex color code，e.g. #FFFFFF、#00D26A"` (Default font color follows system theme)  
    Restart BlueGauge after updating the configuration
3. others: set the icon color to the connection color in `Settings`-`Tray Options` (connected as green, disconnected as red)

<div align="center">
    <img src="screenshots/battery.png" style="width=90%; display:block; margin:0 auto 10px;" />
    <div style="display:flex; justify-content:space-between; width:100%; margin:0 auto;">
        <img src="screenshots/connect.png" style="width:45%; display:block;">
        <img src="screenshots/disconnect.png" style="width:45%; display:block;">
    </div>
</div>

</details>


<details>
<summary>Use ring icon</summary>

1. check the device that needs to display the battery    
2. open tray menu - `Settings` - `Tray Options` - `Icon Style` - `Ring Icon`   
3. set color, open tray menu -- `Settings` -- `Open Config`  
`highlight_color` = `"Hex color code，e.g. #4CD082"`( Default green, highlight color turns red when the device battery is low )    
`background_color` = `"Hex color code，e.g. #DADADA"` ( The default gray color is adjusted according to the system theme, and it is not recommended to modify it )   
    Restart BlueGauge after updating the configuration
4. others: set the icon color to the connection color in `Settings`-`Tray Options`   

<div align="center">
    <div style="display:flex; justify-content:space-between; width:100%; margin:0 auto;">
        <img src="screenshots/ring.png" style="width:48%; display:block;">
        <img src="screenshots/ring_low_battery.png" style="width:48%; display:block;">
    </div>
    <img src="screenshots/ring_custom.png" style="width=90%; display:block; margin:5 auto 10px;" />
</div>

</details>


<details>
<summary>Use battery icon</summary>

Note: Windows 10 users need the Fluent battery icon to download and install [Segoe Fluent Icons](https://aka.ms/SegoeFluentIcons)

1. check the device that needs to display the battery    
2. open tray menu - `Settings` - `Tray Options` - `Icon Style` - `Battery Icon`   
3. others: set the icon color to the connection color in `Settings`-`Tray Options`   

<div align="center">
    <div style="display:flex; justify-content:space-between; width:100%; margin:0 auto;">
        <img src="screenshots/horizontal_battery_icon.png" style="width:48%; display:block;">
        <img src="screenshots/vertical_battery_icon.png" style="width:48%; display:block;">
    </div>
</div>

</details>

<details>
<summary>Use custom icon</summary>

1. create an `assets` folder in the BlueGauge directory
    - Default：add `0.png` to `100.png`   
    - Follow system theme：In the `assets` folder, create the `dark` and `light` folders respectively, and add `0.png` to `100.png` photos respectively
2. restart BlueGauge  

</details>

2. Settings: Show the (connected) devices with the lowest battery

    Note: After setting up, you cannot manually select the device that needs to show the battery level. If you need to manually select the device that needs to show the battery level, please turn off this option.

3. Setting: Bluetooth device name aliases

    1. open tray menu -- `Settings` -- `Open Config`   

    2. Add the required Bluetooth device alias under `[device_aliases]` (note that you use quotation marks to wrap the name)

        - e.g. `"Bluetooth device name" = "Bluetooth alias"`
        - e.g. `"WH-1000XM6" = "Sony Headphones"`
        - e.g. `"Surface Pen" = "Pen"`
        - e.g. `"HUAWEI FreeBuds Pro 5" = "HUAWEI FreeBuds"`

4. Setting: tooltip

    - Shows unconnected devices
    - Truncate devices Name
    - Changing the device power location

5. Setting: notice

    - Low battery notice
    - Notification when reconnecting the device
    - Notification when disconnecting the device
    - Notification when adding a new device
    - Notification when moving a new device
    - Notifications stay on the screen

6. Setting: Auto start

## Download: 


[Github](https://github.com/iKineticate/BlueGauge/releases/latest) ( Please download the x86_64 version by default, and download the ARM version for special systems Windows on ARM. )

## Translation Note

Translations are AI-generated and may contain errors. Please help improve them by [reporting issues](https://github.com/iKineticate/BlueGauge/issues/new/choose) or [submitting corrections](https://github.com/iKineticate/BlueGauge/pulls).


## Known Issues & Suggested Solutions

### 1. Unable to obtain 2.4GHz device battery information

Different 2.4GHz devices have different communication protocols, so it is impossible to obtain battery information uniformly.

**Solution:**

Welcome contributions from developers who can help us extend support for these devices.


### 2. The character length of tray tooltip is currently limited. When the tooltip text exceeds this limit, it gets truncated, which can result in incomplete device names being displayed. This can cause confusion for users, especially when multiple devices are connected.

**Solution:**

1. **Custom Bluetooth Name**：Shorten the length of the name by giving the Bluetooth name alias.

2. **Limit Device Name Length**: Implement a character limit for device names that ensures they fit within the available space of the tray notification. This may require shortening longer names to prevent truncation.

3. **Hide Disconnected Devices**: Consider not displaying disconnected devices in the tray notifications. This approach would reduce clutter and ensure that only relevant information is shown, thereby preventing text overflow.

### 3. How to display the battery level of multiple devices on the tray?

- **Solution:**: Create another folder, copy `BlueGauge.exe` and `BlueGauge.toml` to the folder, then rename `BlueGauge.exe` to another name, and finally turn on and set the display battert level to other Bluetooth device, set the `Launch at Startup`.

### 4. Connection indicator in tray tooltip has no color

Connection indicator only supports displaying colors in Windows 11

### 5. The battery level of the device does not match expectations

It could be that there is a problem with the device reporting (such as a cheap device) that causes only specific values to be reported, such as:

- Only displays multiples of 10%, such as 10%, 20%, 30%...

- Only specific values such as 0%, 50%, and 100% are displayed regularly

## Other Bluetooth battery display software

 - Supports more devices：

    - [MagicPods](https://apps.microsoft.com/detail/9P6SKKFKSHKM) (**Purchase**)   

    - [Bluetooth Battery Monitor](https://www.bluetoothgoodies.com/) (**Purchase**)   

 - Apple: [AirPodsDesktop](https://github.com/SpriteOvO/AirPodsDesktop)

 - Huawei: [OpenFreebuds](https://github.com/melianmiko/OpenFreebuds)

 - Samsung:

    - [Galaxy Buds](https://apps.microsoft.com/detail/9NHTLWTKFZNB)

    - [Galaxy Buds Client](https://github.com/timschneeb/GalaxyBudsClient)  

- Logitech: [elem](https://github.com/Fuwn/elem)   

- SteelSeries Arctis: [Arctis Battery Indicator](https://github.com/aarol/arctis-battery-indicator)   
