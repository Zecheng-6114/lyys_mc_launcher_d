# lyys_mc_launcher

[![License](https://img.shields.io/github/license/Zecheng-6114/lyys_mc_launcher?style=flat-square&logo=gnu)](LICENSE) [![Release](https://img.shields.io/github/v/release/Zecheng-6114/lyys_mc_launcher?include_prereleases&style=flat-square&logo=github)](https://github.com/Zecheng-6114/lyys_mc_launcher/releases) [![Stars](https://img.shields.io/github/stars/Zecheng-6114/lyys_mc_launcher?style=flat-square&logo=github)](https://github.com/Zecheng-6114/lyys_mc_launcher/stargazers) [![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/) [![GPUI Kit](https://img.shields.io/badge/GPUI_Kit-0.10.0-lightblue?style=flat-square)](https://gpui-kit.com/) [![Minecraft](https://img.shields.io/badge/Minecraft-Java_Edition-green?style=flat-square)](https://www.minecraft.net/) ![Platform](https://img.shields.io/badge/Platform-Windows_%7C_Linux-lightgrey?style=flat-square)

[简体中文](README.md) | [English](README.en.md) | [日本語](README.ja.md)

基于 Rust 与 GPUI 的 Minecraft 启动器，提供图形界面与命令行双模式，支持原版及 Fabric / Forge / NeoForge 的下载、安装与启动，并可直接导入 Modrinth、CurseForge、MCBBS 及服务端整合包。

## 功能特性

- **双模式**：无参数启动图形界面，带参数进入命令行模式
- **版本安装**：一键安装原版，或叠加 Fabric / Forge / NeoForge 加载器
- **版本继承链**：完整解析 `inheritsFrom` 依赖链并合并库与启动参数
- **整合包导入**：支持 Modrinth（`.mrpack`）、CurseForge、MCBBS 与服务端整合包，支持 `--dry-run` 预览
- **模组管理**：通过 Modrinth API 搜索并安装 mod，自动处理依赖（可选依赖可控）；支持列出、启停与删除已装模组
- **实例隔离**：每个实例可独立使用自己的 `versions/<实例>` 目录作为游戏目录
- **Java 探测**：自动扫描本机 Java，支持添加自定义扫描目录，按实例所需版本选择运行环境；`java list` / `java path` / `java add` 可查看与维护结果
- **镜像加速**：`--mirror` / `--maven` 可重写 Mojang 资源与 Maven 库下载源
- **完整性校验**：下载文件带 SHA1 / SHA512 校验与重试
- **离线启动**：由用户名派生离线 UUID
- **正版登录**：微软设备码流程登录，验证码自动复制并打开浏览器，令牌自动续期；已登录时启动自动使用正版身份，`--offline` / `--username` 可强制离线
- **账号管理**：图形界面「账号」页可添加微软账号与离线账号，切换当前账号或删除；启动时自动使用所选账号
- **多语言**：简体中文 / English / 日本語
- **任务并行**：图形界面支持多任务并行执行与实时日志

## 环境要求

- Rust 1.85 或更高版本（edition 2024）
- Windows / Linux；Linux 构建需 `cmake`、`clang`、`pkg-config` 及 Wayland / X11 相关开发库（详见 `.github/workflows/release.yml`）

## 构建

```bash
cargo build --release
```

产物位于 `target/release/lyys_mc_launcher`（Windows 为 `lyys_mc_launcher.exe`）。

## 使用

### 图形界面

直接运行可执行文件（不带参数）即可打开图形界面。

### 命令行

```text
launch:   lyys_mc_launcher launch --dir <game_dir> --name <instance> [--username <name>] [--memory <MB>] [--isolate] [--offline] [--mirror <url>] [--maven <url>] [--java <path>]
download: lyys_mc_launcher download --dir <game_dir> --version <version> [--loader <fabric|forge|neoforge>] [--loader-version <v>] [--name <instance>] [--java <path>] [--mirror <url>] [--maven <url>]
search:   lyys_mc_launcher search --query <text> --type <mod|modpack|resourcepack|shader|datapack> [--loader <a,b>] [--version <x,y>] [--limit <n>]
add:      lyys_mc_launcher add --dir <game_dir> --name <instance> --slug <slug> --type <kind> [--version <id|number>] [--with-optional]
import:   lyys_mc_launcher import --dir <game_dir> (--file <archive> | --url <url> | --modrinth <slug>) [--pack-version <id>] [--name <instance>] [--java <path>] [--loader <fabric|forge|neoforge>] [--loader-version <v>] [--with-optional] [--dry-run] [--mirror <url>] [--maven <url>]
mods:     lyys_mc_launcher mods list|remove|toggle --dir <game_dir> --name <instance> [--mod <id,file,...>]
java:     lyys_mc_launcher java list [--refresh] | java path --version <mc_version> | java add|remove --path <path>
login:    lyys_mc_launcher login
logout:   lyys_mc_launcher logout
account:  lyys_mc_launcher account use|remove <name|uuid> | account add-offline <name>
```

示例：

```bash
# 下载并安装 1.21.4 原版实例
lyys_mc_launcher download --dir D:\game\minecraft --version 1.21.4

# 在该实例上叠加 Fabric 并启动
lyys_mc_launcher download --dir D:\game\minecraft --version 1.21.4 --loader fabric --name 1.21.4-fabric
lyys_mc_launcher launch --dir D:\game\minecraft --name 1.21.4-fabric --username Steve --isolate

# 微软正版登录（浏览器打开链接并输入代码后自动完成）
lyys_mc_launcher login

# 添加离线账号并设为当前账号
lyys_mc_launcher account add-offline Steve

# 在各账号之间切换
lyys_mc_launcher account use Steve

# 从 Modrinth 导入整合包
lyys_mc_launcher import --dir D:\game\minecraft --modrinth <pack-slug>

# 列出实例已装模组（remove / toggle 同理）
lyys_mc_launcher mods list --dir D:\game\minecraft --name <instance>

# 列出已探测的 Java，或按游戏版本选择 Java
lyys_mc_launcher java list
lyys_mc_launcher java path --version 1.20.1

# 添加自定义目录（含多个 JDK 的父目录也可），之后自动检测会一并扫描
lyys_mc_launcher java add --path D:\tools\java
```

## 配置

全局配置位于 `<基目录>/lyys_mc_launcher/config.json`：

| 字段 | 说明 |
|---|---|
| `game_dir` | 默认游戏根目录 |
| `memory` | 内存上限（MB） |
| `java` | Java 可执行文件路径 |
| `mirror` | Mojang 资源镜像 |
| `maven` | Maven 库镜像 |
| `isolate` | 是否默认开启实例隔离 |
| `language` | 界面语言（`zh-CN` / `en-US` / `ja-JP`） |
| `user_java` | 手动添加的 Java 扫描路径（可执行文件、Java 主目录或包含多个 JDK 的父目录） |

基目录的确定规则：从可执行文件所在目录逐级向上查找包含 `Cargo.toml` 的目录；开发环境即为项目根目录，否则使用当前工作目录。

每个实例还可拥有独立配置：

- `versions/<实例>/launch.json`：实例级启动参数（用户名、内存、Java、镜像、隔离）
- `versions/<实例>/lyys-instance.json`：实例元数据（隔离标记、整合包来源记录）
- `<基目录>/lyys_mc_launcher/accounts.json`：账号列表与当前账号（微软令牌与离线账号，由 `login` / `account` 写入，启动时自动使用当前账号）

运行日志写入 `<基目录>/lyys_mc_launcher/log/`。

## 项目结构

```text
src/
├── main.rs        # 入口：CLI 分发或 GUI 启动
├── cli.rs         # 命令行解析
├── gui.rs         # GPUI 图形界面
├── i18n.rs        # 多语言支持
├── locales/       # 多语言资源（en-US / zh-CN / ja-JP）
├── version.rs     # 版本 JSON 解析与继承链合并
├── download.rs    # 下载、校验、镜像重写
├── loader.rs      # Fabric / Forge / NeoForge 安装
├── installer.rs   # Forge 安装器 processor 执行
├── metadata.rs    # 加载器版本工件解析
├── modrinth.rs    # Modrinth 搜索与安装
├── mods.rs        # 已装模组列出、启停与删除
├── modpack.rs     # 整合包导入（modpack/ 为各 provider 实现）
├── instance.rs    # 实例配置与隔离
├── launch.rs      # 启动参数构建
├── microsoft.rs   # 微软正版登录（设备码流程）
└── account.rs     # 离线 UUID
```

## 许可证

本项目基于 [GPL-3.0](LICENSE) 许可证发布。

## 声明

LYYS MC Launcher 是一个非官方的 Minecraft 启动器，与 Mojang Studios、Microsoft 或他们位于中国大陆的代理公司之间并无任何从属或关联。
