# Windows CI 专用：强制 C++ 依赖用静态 CRT（/MT）编译。
#
# 背景：sherpa-onnx 的 Windows 预编译库是 static-MT（/MT）打包的。若
# llama.cpp 用默认的动态 CRT（/MD），同一个 exe 里会出现
# LNK2005（std 符号重复定义）与 LNK2038（RuntimeLibrary 不匹配）。
#
# 为什么不能只靠 LLAMA_STATIC_CRT=1：llama-cpp-sys-2 把 /MT 经
# cmake-rs 写进 CMAKE_C_FLAGS[_RELEASE]，但 llama.cpp 的
# cmake_minimum_required(VERSION 3.14...3.28) 使 CMP0091 为 NEW，
# CMake 改由 CMAKE_MSVC_RUNTIME_LIBRARY 决定运行库（默认 /MD）并
# 覆盖用户标志。因此必须在 toolchain 文件里直接设置该变量
# （toolchain 在 project() 之前生效，优先级最高）。
#
# 通过环境变量 CMAKE_TOOLCHAIN_FILE 引用本文件（CMake ≥ 3.21 原生支持，
# windows-2022 runner 自带 cmake 3.31+）。只应作用于 Windows CI：
# 本文件一旦被设为 toolchain 会对该构建树内所有 C/C++ 目标生效。
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
