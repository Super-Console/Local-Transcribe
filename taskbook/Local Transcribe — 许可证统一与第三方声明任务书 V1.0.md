# Local Transcribe — 许可证统一与第三方声明任务书 V1.0

> **性质**：合规收尾（第二十一份）。已决定采用 **MIT**。
> **不得修改其它任务书。**
> **前置**：可与发布任务同轮，但必须在「打包」之前完成第 1–3 节，否则安装包里不会带声明。

---

## 1. 统一许可证元数据

现状不一致：

| 位置 | 现在 | 改成 |
| --- | --- | --- |
| `LICENSE` | MIT，Copyright 2026 TimmySheep | 保持，核对年份与持有者 |
| `apps/desktop/package.json` | `MIT` | 保持 |
| `crates/transcription-core/Cargo.toml` | `UNLICENSED` | **`MIT`** |
| `crates/transcription-service/Cargo.toml` | `UNLICENSED` | **`MIT`** |
| `crates/transcription-cli/Cargo.toml` | `UNLICENSED` | **`MIT`** |
| `crates/transcription-mcp/Cargo.toml` | `UNLICENSED` | **`MIT`** |
| `crates/lan-compute/Cargo.toml` | `UNLICENSED` | **`MIT`** |
| 根 `Cargo.toml`（workspace） | 无 license 字段 | 加 `license = "MIT"`（workspace 级） |
| `apps/desktop/src-tauri/tauri.conf.json` | 无 copyright | 加 `bundle.copyright: "Copyright © 2026 TimmySheep"` |

**断言**：`grep -rn "UNLICENSED" --include=Cargo.toml .` 结果为空。

---

## 2. README 补许可与商标说明（双语）

在 `README.md` 与 `README.zh-CN.md` 末尾加一小节：

```markdown
## License

Code is licensed under the MIT License — see [LICENSE](LICENSE).

The name "Local Transcribe" and the application icon are not covered by this
license; they remain the property of the author.
```

中文版对应表述。**只加这一段，不要重写其它部分。**

---

## 3. 生成 `THIRD-PARTY-NOTICES.md`

**不要安装任何全局工具。** 用现有的东西：

1. Rust 依赖：`cargo metadata --format-version 1 --all-features` 输出 JSON，
   用 `python3` 解析出 `packages[].{name, version, license, repository}`，
   只列**实际进入二进制的依赖**（即 workspace 成员的依赖闭包，或至少列全部直接依赖 + 全部非 dev 依赖）
2. 前端依赖：读 `apps/desktop/package.json` 的 `dependencies`（不含 devDependencies）
3. 生成 `THIRD-PARTY-NOTICES.md`，按许可证分组（MIT / Apache-2.0 / MPL-2.0 / BSD / 其它），
   每组按名称排序，格式：

```
### MIT
- axum 0.8.x — https://…
- tokio 1.x — https://…

### Apache-2.0
- sherpa-onnx — https://github.com/k2-fsa/sherpa-onnx
- tauri 2.x — https://…
```

**必须点名的两条**（即使已在上面的列表里，也要在文件开头单独写一段说明）：

- **sherpa-onnx**：Apache-2.0；本项目通过 `sherpa-onnx-sys` 静态链接其运行库
- **symphonia**：**MPL-2.0**（文件级 copyleft）。本项目未修改其源码；若将来修改，
  被修改的文件需按 MPL-2.0 公开

**断言**：文件存在；包含字符串 `sherpa-onnx`、`symphonia`、`MPL-2.0`、`Apache-2.0`。

---

## 4. 把声明带进安装包

- `tauri.conf.json` 增加 `bundle.resources`，把 `THIRD-PARTY-NOTICES.md` 与 `LICENSE`
  打进去（路径按 Tauri 2 的写法，确认它们在 `.app/Contents/Resources/` 下）
- 应用「设置 → 关于」或设置页底部，加一个能打开该文件的入口（没有关于页就加一行链接）
- 发布资产里也附一份 `THIRD-PARTY-NOTICES.md`

**断言**：打包后 `ls -l "…/Local Transcribe.app/Contents/Resources/"` 能看到两份文件；
`codesign --verify --deep --strict` 仍通过。

---

## 5. 核实 7 个模型的许可证（不许猜）

清单里的模型（`apps/desktop/src-tauri/src/model_download.rs`）：

| repository |
| --- |
| `csukuangfj2/sherpa-onnx-x-asr-zipformer-transducer-zh-en-int8-2026-06-03` |
| `csukuangfj2/sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26` |
| `csukuangfj2/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25` |
| `csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09` |
| `csukuangfj/sherpa-onnx-paraformer-zh-2023-09-14` |
| `csukuangfj/sherpa-onnx-zipformer-ctc-small-zh-int8-2025-07-16` |
| `csukuangfj/sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12` |

逐个打开 Hugging Face 上的模型卡，记录：

- 模型卡上标注的 license
- 若**没有**标注，写 `未标注`
- 若为**非商业**或其它限制性许可，单独标红并写清影响（例如：不可随安装包分发、不可商用）

写入新建的 `docs/model-licenses.md`，每条包含：模型名、仓库 URL、license、核实日期、备注。

**禁止**：凭印象填、把"出处是 sherpa-onnx 官方导出"当成许可证依据。

> 若某个模型确实受限，**先记录下来并报告**，不要自行删除清单条目——删哪个由 Timmy 定。

---

## 6. 不做

- 不改依赖版本、不换库
- 不改模型清单（只记录，不删）
- 不写"已获授权"之类未发生的声明
- 不评价观感、不截图

---

## 7. 第一条命令

```sh
cd /Users/TimmySheep/AWS/VoiceToTextApp && \
  head -3 LICENSE && \
  grep -rn "UNLICENSED" --include=Cargo.toml . | wc -l && \
  grep -n "repository:" apps/desktop/src-tauri/src/model_download.rs | head -8 && \
  cargo metadata --format-version 1 --no-deps | head -c 200
```
