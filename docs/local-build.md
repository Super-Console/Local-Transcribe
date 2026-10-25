# 本机构建（macOS）

记录在本机（Apple Silicon Mac）从零打包桌面应用时遇到的构建链问题与绕行方式。
Windows 构建链见 `docs/windows-build-log.md`。

## release 构建必须 `CARGO_PROFILE_RELEASE_STRIP=none`

**现象**（2026-10-24 实测，Xcode 21.0.0 / ld-27037.1，Rust 1.99）：

从零执行 release 构建（`cargo build --release` 或 `npm run tauri build --bundles app`）时，
部分 proc-macro crate 的 dylib（`phf_macros`、`derive_more_impl`、`serde_derive`、
`zerofrom_derive`、`thiserror_impl` 等）被链接器写坏：Mach-O 头正常，但
`LC_SYMTAB`/`LC_DYLD_INFO` 的 string pool 偏移未按 8 字节对齐，dyld 拒绝加载
（`dlopen ... mis-aligned LINKEDIT string pool`）。rustc 随后报
`error[E0463]: can't find crate for ...`，构建失败。

**绕行**：

```sh
CARGO_PROFILE_RELEASE_STRIP=none npm run tauri build --bundles app --no-sign
```

加该环境变量后（不 strip，符号保留），构建与运行均正常；产出的 `.app` 更大，
仅供本机安装验证。

**注意事项**：

- 这是本机构建链问题，**不要修改仓库的 release profile 或 CI 配置去迁就本机**；
  CI 上的 Windows 构建不受影响，已发布的 release 资产也不受影响。
- `target/` 中的 `sherpa-onnx-prebuilt/` 目录是构建必需的缓存（离线环境无法
  重新下载 sherpa-onnx 预编译库）；清理构建产物时必须保留它。

## 另一个本机链接缺失：`-lclang_rt.osx`

**现象**（2026-10-25，Xcode 27.1 / clang 21.0.0，Rust 1.97.1）：

release 链接报 `Undefined symbols: ___isPlatformVersionAtLeast`，引用来自
`ggml-metal-device.m.o`。该符号不在 `libSystem` 中，只在 Xcode 的
`libclang_rt.osx.a`；Rust 在 darwin 上用 `-nodefaultlibs`，clang 不会再自动带上它。

**绕行**（本机专用，不写进仓库）：

```sh
RES=$(clang -print-resource-dir)
CARGO_PROFILE_RELEASE_STRIP=none \
RUSTFLAGS="-C link-arg=-L$RES/lib/darwin -C link-arg=-lclang_rt.osx" \
  npm run bundle:mac
```

## `bundle:mac` 必须重建节点 sidecar

`tauri.conf.json` 的 `externalBin` 只打包
`src-tauri/binaries/local-transcribe-node-aarch64-apple-darwin` 这个**预置文件**，
`tauri build` 不会重建它。若桌面端新增了节点参数而 sidecar 没更新，应用内启动
节点会以退出码 2 打印帮助（2026-10-25 的 `--llm-models-dir` 事故）。
因此 `bundle:mac` 现在先 `cargo build --release -p voice-to-text-lan --bin
local-transcribe-node` 并 `cp` 覆盖 sidecar，再执行 `tauri build`；
`apps/desktop/tests/bundle-sidecar.test.mjs` 会检查这个顺序。
