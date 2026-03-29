# BlueGauge

A lightweight tray tool for easily viewing the battery level of your Bluetooth devices.

一款轻便的托盘工具，可轻松查看蓝牙设备的电池电量。

<p align="center">
	<a href="https://github.com/iKineticate/BlueGauge/releases/latest"><img alt="GitHub Release" src="https://img.shields.io/github/v/release/iKineticate/BlueGauge?"></a>
	<a href="https://github.com/iKineticate/BlueGauge/releases/latest"><img alt="Github Downloads" src="https://img.shields.io/github/downloads/iKineticate/BlueGauge/total?logo=github"></a>
    <a href="https://wwxv.lanzoul.com/b009hchxrc"><img alt="Static Badge" src="https://img.shields.io/badge/蓝奏云-2K%2B-blue?logo=icloud"></a>
    <img alt="Language" src="https://img.shields.io/badge/build-Rust-yellow?logo=rust">
    <a href="https://github.com/iKineticate/BlueGauge#"><img alt="GitHub License" src="https://img.shields.io/github/license/iKineticate/BlueGauge"></a>
</p>

<div align="center">
  <img src="screenshots/app.png" alt="App Screenshot" style="width: 100%; max-width: 100%; height: auto; display: block;" />
</div>

<h3 align="center"> 简体中文 | <a href='./README-en.md'>English</a></h3>

## 功能

1. 设置：蓝牙设备电量作为托盘图标  

<details>
<summary>使用数字图标（默认）</summary>

1. 勾选需显示电量设备    
2. 可选设置相关参数，打开托盘菜单 - `设置` - `打开配置`  
`font_name` = `"系统字体名称，如 Microsoft YaHei UI"`（默认 `Arial`）  
`font_color` = `"十六进制颜色代码，如 #FFFFFF、#00D26A"`（默认字体颜色跟随系统主题）  
  更改配置文件后，重新启动 BlueGauge
3. 其他：图标颜色支持连接配色，在`设置`-`托盘选项`-`设置图标颜色为连接配色`（已连接为绿色，断开连接为红色）

<div align="center">
    <img src="screenshots/battery.png" style="width=90%; display:block; margin:0 auto 10px;" />
    <div style="display:flex; justify-content:space-between; width:100%; margin:0 auto;">
        <img src="screenshots/connect.png" style="width:45%; display:block;">
        <img src="screenshots/disconnect.png" style="width:45%; display:block;">
    </div>
</div>

</details>
 

<details>
<summary>使用圆环图标</summary>

1. 勾选需显示电量设备    
2. 打开托盘菜单 - `设置` - `托盘选项` - `图标样式` - `圆环图标`
3. 可选设置相关参数，打开托盘菜单 - `设置` - `打开配置`   
`highlight_color`（电量颜色） = `"十六进制颜色代码，如 #4CD082"`（默认绿色，当设备低电量时为红色）    
`background_color`（无电量颜色） = `"十六进制颜色代码，如 #DADADA"`（默认灰色随系统主题调整）   
    更改配置文件后，重新启动 BlueGauge 
4. 其他：图标颜色支持连接配色，在`设置`-`托盘选项`-`设置图标颜色为连接配色`

<div align="center">
    <div style="display:flex; justify-content:space-between; width:100%; margin:0 auto;">
        <img src="screenshots/ring.png" style="width:48%; display:block;">
        <img src="screenshots/ring_low_battery.png" style="width:48%; display:block;">
    </div>
    <img src="screenshots/ring_custom.png" style="width=90%; display:block; margin:5 auto 10px;" />
</div>
</details>


<details>
<summary>使用电池图标</summary>

注意：Windows10系统用户需要 Fluent 电池图标，可下载并安装 [Segoe Fluent Icons](https://aka.ms/SegoeFluentIcons)
1. 勾选需显示电量设备    
2. 打开托盘菜单 - `设置` - `托盘选项` - `图标样式` - `电池图标`
3. 其他：图标颜色支持连接配色，在`设置`-`托盘选项`-`设置图标颜色为连接配色`

<div align="center">
    <div style="display:flex; justify-content:space-between; width:100%; margin:0 auto;">
        <img src="screenshots/horizontal_battery_icon.png" style="width:48%; display:block;">
        <img src="screenshots/vertical_battery_icon.png" style="width:48%; display:block;">
    </div>
</div>

</details>


<details>
<summary>使用自定义图标</summary>

1. 在软件目录下创建一个 `assets` 文件夹，
    - 跟随系统主题：在 `assets` 文件夹中，分别创建 `dark` 和 `light` 文件夹，并分别添加 `0.png` 至 `100.png` 照片
    - 不跟随系统主题：在 `assets` 文件夹中添加 `0.png` 至 `100.png` 照片  
2. 重新启动 BlueGauge

</details>

2. 设置：显示最低电量的（已连接）设备

    注意：设置后无法手动选择需显示电量的设备，若需要手动选择需显示电量设备请关闭这个选项

3. 设置：蓝牙设备名称别名

    1. 打开托盘菜单-`设置`-`打开配置`   

    2. 在`[device_aliases]`下方添加需要别名的蓝牙设备（注意使用英文引号包裹名称）

        - 例如 `"蓝牙设备名称" = "蓝牙别名"`
        - 例如 `"WH-1000XM6" = "Sony Headphones"`
        - 例如 `"NuPhy Air60 V2-1" = "NuPhy Air60"`
        - 例如 `"HUAWEI FreeClip 2" = "HUAWEI FreeClip"`

4. 设置：托盘提示

    - 显示未连接的设备
    - 限制设备名称长度
    - 更改设备电量位置

5. 设置：通知

    - 低电量时通知
    - 重新连接时通知
    - 断开连接时通知
    - 添加设备时通知
    - 移除设备时通知
    - 通知常驻屏幕

6. 设置：开机自启动 

## 下载

默认请下载 **x86_64** 版本，特殊系统 Windows on ARM 下载 arm 版本

1. [Github](https://github.com/iKineticate/BlueGauge/releases/latest)

2. [蓝奏云](https://wwxv.lanzoul.com/b009hchxrc)（密码：6666）

## 已知问题与建议

### 1. 无法获取 2.4GHz 设备电量信息

不同的 2.4GHz 设备的通信协议不同，无法做到统一获取电量信息，需要针对不同设备进行不同的优化。

- **解决方案：**: 欢迎有能力的开发者贡献代码或提供思路，帮助扩展对这些设备的支持。

### 2. 托盘提示内容不全

托盘提示的字符长度有限，当设备过多和（或）设备名称过长时，提示文本会被截断，导致无法完整显示设备信息。

**建议的解决办法：**

1. **自定义蓝牙设备名称**：通过给蓝牙名称别名缩短其名称长度。

2. **限制设备名称长度**：对设备名称的字符长度进行限制，确保其在托盘通知区域内完整显示。

3. **隐藏未连接的设备**：对于未连接的设备，可以考虑不在托盘通知中显示，从而减少杂乱，避免文本溢出。

### 3. 怎么在托盘显示多个设备电量？

- **解决方案：**: 另外创建一个文件夹，并复制 `BlueGauge.exe` 和 `BlueGague.toml`到该文件夹，然后重命名 `BlueGauge.exe` 为其他名称（不建议使用中文命名），最后打开并设置显示电量为其他蓝牙设备、开机自启动等设置即可。

### 4. 托盘提示中的连接指示器无颜色

托盘提示中的带颜色连接指示器仅支持 Windows11

### 5. 设备电量与预期不符

可能是设备报告存在问题（如廉价设备），导致只能报告特定值，例如：

 - 仅显示10%的倍数，如10%、20%、30%...

 - 只固定显示0%、50%、100%这几个特定数值

## 其他蓝牙电量软件

 - 支持较多设备：

    - [MagicPods](https://apps.microsoft.com/detail/9P6SKKFKSHKM) (**付费**)   

    - [Bluetooth Battery Monitor](https://www.bluetoothgoodies.com/) (**付费**)   

 - 苹果：[AirPodsDesktop](https://github.com/SpriteOvO/AirPodsDesktop)

 - 华为：[OpenFreebuds](https://github.com/melianmiko/OpenFreebuds)

 - 三星：  

    - [Galaxy Buds](https://apps.microsoft.com/detail/9NHTLWTKFZNB)   

    - [Galaxy Buds Client](https://github.com/timschneeb/GalaxyBudsClient)  

- 罗技: [elem](https://github.com/Fuwn/elem)   

- 赛睿: [Arctis Battery Indicator](https://github.com/aarol/arctis-battery-indicator)   
