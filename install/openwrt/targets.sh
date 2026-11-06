#!/usr/bin/env bash
# 官方 25.12 的 64 位架构标签；ARM64 共用基线 ARMv8 musl 二进制。
OPENWRT_ARCHES=(x86_64 aarch64_generic aarch64_cortex-a53 aarch64_cortex-a72 aarch64_cortex-a76)

openwrt_binary_arch() {
  case "$1" in
    x86_64) printf '%s\n' x86_64 ;;
    aarch64_generic|aarch64_cortex-a53|aarch64_cortex-a72|aarch64_cortex-a76) printf '%s\n' aarch64 ;;
    *) printf '不支持的 OpenWrt 架构：%s\n' "$1" >&2; return 1 ;;
  esac
}
