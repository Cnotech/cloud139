#!/usr/bin/env python3
"""Local-only E2E fixtures and content oracle; never invokes cloud139 or a network client."""
import argparse
import hashlib
import json
import os
from pathlib import Path


def reject_links(path):
    for current in (path, *path.parents):
        if current.is_symlink():
            raise ValueError(f"Refusing symlink: {current}")


def initialize(root):
    reject_links(root)
    if root.exists() and any(root.iterdir()):
        raise ValueError("Fixture directory must be new or empty")
    root.mkdir(parents=True, exist_ok=True)
    files = {
        "files/a.txt": b"one\n",
        "files/b.txt": b"two\n",
        "files/blocker.bin": b"blocker\n",
        "other-files/a.txt": b"old target\n",
        "tree/root.txt": b"root payload\n",
        "tree/nested/note with spaces.txt": b"nested payload\n",
        "tree/nested/deep/data.bin": bytes(range(256)) * 1024,
        "same-v1/same.bin": b"AAAA",
        "same-v2/same.bin": b"BBBB",
        "duplicates/left/shared.txt": b"left",
        "duplicates/right/shared.txt": b"rght",
        "tree-as-file/tree": b"file blocking a directory\n",
        "sync-source/file1.txt": b"first\n",
        "sync-source/file2.txt": b"second\n",
        "sync-source/subdir/sub.txt": b"sub\n",
    }
    for index in range(1, 122):
        files[f"paged/page-{index:03}.txt"] = f"root {index}\n".encode()
        files[f"paged/child/page-{index:03}.txt"] = f"child {index}\n".encode()
    for name, data in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    for name in ("tree/empty", "tree/nested/empty", "paged/empty", "empty", "sync-source/empty"):
        (root / name).mkdir(parents=True, exist_ok=True)


def manifest(root):
    reject_links(root)
    if not root.is_dir():
        raise ValueError("Manifest root must be an existing directory")
    directories = []
    files = {}
    for parent, child_dirs, child_files in os.walk(root, followlinks=False):
        parent = Path(parent)
        directories.append(parent.relative_to(root).as_posix())
        for name in child_dirs:
            reject_links(parent / name)
        for name in child_files:
            path = parent / name
            reject_links(path)
            if not path.is_file():
                raise ValueError(f"Refusing special file: {path}")
            digest = hashlib.sha256()
            size = 0
            with path.open("rb") as source:
                for block in iter(lambda: source.read(1024 * 1024), b""):
                    size += len(block)
                    digest.update(block)
            files[path.relative_to(root).as_posix()] = {"size": size, "sha256": digest.hexdigest()}
    return {"directories": sorted(directories), "files": dict(sorted(files.items()))}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("init")
    create.add_argument("root", type=Path)
    scan = commands.add_parser("manifest")
    scan.add_argument("root", type=Path)
    scan.add_argument("--output", type=Path, required=True)
    compare = commands.add_parser("compare")
    compare.add_argument("expected", type=Path)
    compare.add_argument("actual", type=Path)
    args = parser.parse_args()
    if args.command == "init":
        initialize(args.root.absolute())
    elif args.command == "manifest":
        reject_links(args.output.absolute())
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(manifest(args.root.absolute()), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    else:
        expected = json.loads(args.expected.read_text(encoding="utf-8"))
        actual = json.loads(args.actual.read_text(encoding="utf-8"))
        if expected != actual:
            parser.exit(1, "Directory manifest mismatch (paths, empty directories, sizes or SHA256)\n")


if __name__ == "__main__":
    main()
