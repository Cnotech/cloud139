import os
from pathlib import Path
import subprocess


MAX_LINES = 500
ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    files = subprocess.check_output(
        [
            "git",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            ":(glob)packages/*/src/**/*.rs",
            ":(exclude,glob)packages/*/src/tests/**",
        ],
        cwd=ROOT,
    )
    for name in files.rstrip(b"\0").split(b"\0"):
        path = Path(os.fsdecode(name))
        source = ROOT / path
        if not source.is_file():
            continue
        with source.open("rb") as contents:
            lines = sum(1 for _ in contents)
        if lines <= MAX_LINES:
            continue

        message = f"Rust 源文件有 {lines} 行，超过 {MAX_LINES} 行，请检查是否可以按职责拆分"
        if os.getenv("GITHUB_ACTIONS") == "true":
            file = path.as_posix().replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
            file = file.replace(":", "%3A").replace(",", "%2C")
            print(f"::warning file={file},line={MAX_LINES + 1}::{message}")
        else:
            print(f"warning: {path}: {message}")


if __name__ == "__main__":
    main()
