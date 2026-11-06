#!/usr/bin/env bash
# 稳定前只发布 A53；其他架构映射保留供后续扩展使用。
OPENWRT_ARCHES=(aarch64_cortex-a53)

openwrt_binary_arch() {
  case "$1" in
    x86_64) printf '%s\n' x86_64 ;;
    aarch64_generic|aarch64_cortex-a53|aarch64_cortex-a72|aarch64_cortex-a76) printf '%s\n' aarch64 ;;
    *) printf '不支持的 OpenWrt 架构：%s\n' "$1" >&2; return 1 ;;
  esac
}
