#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
max_lines=500

git ls-files --cached --others --exclude-standard -z -- \
    ':(glob)packages/*/src/**/*.rs' \
    ':(exclude,glob)packages/*/src/tests/**' |
    while IFS= read -r -d '' file; do
        if [[ ! -f $file ]]; then
            continue
        fi

        lines=$(LC_ALL=C awk 'END { print NR }' < "$file")
        if (( lines <= max_lines )); then
            continue
        fi

        message="Rust 源文件有 $lines 行，超过 $max_lines 行，请检查是否可以按职责拆分"
        if [[ ${GITHUB_ACTIONS:-} == true ]]; then
            file=${file//%/%25}
            file=${file//$'\r'/%0D}
            file=${file//$'\n'/%0A}
            file=${file//:/%3A}
            file=${file//,/%2C}
            printf '::warning file=%s,line=%s::%s\n' "$file" "$((max_lines + 1))" "$message"
        else
            printf 'warning: %s: %s\n' "$file" "$message"
        fi
    done
