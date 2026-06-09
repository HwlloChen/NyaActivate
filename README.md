# NyaActivate

恶搞模拟 Windows 激活水印的 Rust 程序，非常适合各电教临走前给下一届留个好东西喵 <img width="64" height="64" alt="image" src="https://github.com/user-attachments/assets/50988773-a5e4-434f-bf0f-729cf01d02ed" />

代码由AI完成，经过人工测试。

## 截图

<img width="788" height="246" alt="image" src="https://github.com/user-attachments/assets/55fdd6d8-127f-4135-98bd-4043bd7202cc" />

<img width="639" height="297" alt="image" src="https://github.com/user-attachments/assets/794cc4c0-07df-4764-b89f-7e8b15fc787e" />


## 配置

`config.toml` 与 `nya-activate.exe` 同目录，不存在则使用默认值。

| 字段         | 默认值                         | 说明                       |
| ------------ | ------------------------------ | -------------------------- |
| `line1`      | `"激活 Windows"`               | 第一行文字                 |
| `line2`      | `"转到"设置"以激活 Windows。"` | 第二行文字                 |
| `color`      | `"#a6a7a8"`                    | 字体颜色，16 进制          |
| `font_size1` | `18`                           | 第一行字号（pt）           |
| `font_size2` | `13`                           | 第二行字号（pt）           |
| `bold`       | `false`                        | 是否加粗字体               |
| `colorful`   | `false`                        | 是否启用彩虹渐变           |
| `level`      | `"TopMost"`                    | `"TopMost"`: 强制最上层 或 `"Desktop"`: 在桌面上(防止TopMost太似冯了喵) |

### 示例

```toml
[watermark]
line1 = "激活 Windows"
line2 = "转到\"设置\"以激活 Windows 喵~"
color = "#a6a7a8"
font_size1 = 19
font_size2 = 14
bold = false
colorful = false
level = "TopMost"
```

## 命令

| 命令                                 | 说明                             |
| ------------------------------------ | -------------------------------- |
| `nya-activate.exe run`               | 前台运行水印（测试用）           |
| `nya-activate.exe service install`   | 安装为 Windows 服务（开机自启）  |
| `nya-activate.exe service uninstall` | 卸载服务                         |
| `nya-activate.exe service status`    | 查看服务状态                     |
| `nya-activate.exe service run`       | 服务入口（SCM 调用，勿手动运行） |

> `service install / uninstall / status` 需要管理员权限。  
> 对于新版Windows, 可以使用 `sudo` 命令

## 构建

```powershell
cargo build --release
```

产物在 `target/release/nya-activate.exe`。
