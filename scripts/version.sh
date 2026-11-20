#!/usr/bin/env bash
# 打印项目版本号：根 Cargo.toml 的 [workspace.package] version。
# CI 里 5 个 job 都用它，别各写一份 sed（版本号改到 workspace 后 core/Cargo.toml 里就没有了）。
set -euo pipefail
sed -n 's/^version = "\(.*\)"/\1/p' "$(dirname "$0")/../Cargo.toml" | head -1
