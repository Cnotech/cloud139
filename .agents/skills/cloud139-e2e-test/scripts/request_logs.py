#!/usr/bin/env python3
"""归档整轮 cloud139 日志，分析用例主命令的请求。"""

import argparse
from collections import Counter, defaultdict
import json
import math
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time


COMMANDS = {"login", "ls", "upload", "download", "rm", "mkdir", "mv", "cp", "rename", "sync"}
CASE_LABEL = re.compile(r"[a-zA-Z0-9_.-]+")


def save_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def run(args):
    argv = args.command_args
    if argv and argv[0] == "--":
        argv = argv[1:]
    if not argv or not CASE_LABEL.fullmatch(args.case):
        raise ValueError("执行命令需要用例编号和 cloud139 参数")
    directory = Path(tempfile.mkdtemp(prefix="cmd-", dir=args.directory))
    metadata = {
        "case": args.case,
        "purpose": args.purpose,
        "command": next((value for value in argv if value in COMMANDS), "unknown"),
        "expected_exit": args.expected_exit,
        "execute_list_limit": args.execute_list_limit,
        "request_limits": args.request_limit,
        "started_ms": time.time_ns() // 1_000_000,
        "exit": None,
    }
    path = directory / "invocation.json"
    save_json(path, metadata)
    # 参数可能含登录凭据，只交给被测进程。保留原有的 stdout 和 stderr。
    result = subprocess.run(
        [args.bin, "--config", args.config, "--log-file", str(directory / "cli.log"), *argv]
    )
    metadata["exit"] = result.returncode if result.returncode >= 0 else 128 - result.returncode
    save_json(path, metadata)
    return metadata["exit"]


def fields(line):
    return dict(field.split("=", 1) for field in line.split() if "=" in field)


def peak_per_second(timestamps):
    ordered = sorted(timestamps)
    first = 0
    peak = 0
    for last, current in enumerate(ordered):
        while current - ordered[first] >= 1_000:
            first += 1
        peak = max(peak, last - first + 1)
    return peak


def archive_header(directory, metadata, merged):
    merged.write(
        f"\n--- invocation={directory.name} case={metadata['case']} "
        f"purpose={metadata['purpose']} command={metadata['command']} exit={metadata['exit']} ---\n"
    )


def read_case(directory, metadata, merged, endpoint_stats, findings):
    invocation = directory.name

    def flag(kind, **details):
        findings.append({"invocation": invocation, "case": metadata["case"], "kind": kind, **details})

    if metadata["exit"] != metadata["expected_exit"]:
        flag("unexpected_exit", expected=metadata["expected_exit"], actual=metadata["exit"])

    counts = Counter()
    levels = Counter()
    pending = {}
    seen_ids = set()
    completed_phases = set()
    timestamps = defaultdict(list)
    phase = "command"
    path = directory / "cli.log"
    if not path.is_file():
        flag("missing_log")
    else:
        with path.open(encoding="utf-8", errors="replace") as log:
            for number, line in enumerate(log, 1):
                merged.write(line)
                parts = line.split(maxsplit=2)
                if len(parts) < 3:
                    continue
                levels[parts[1]] += 1
                if parts[1] != "debug":
                    continue
                message = parts[2]
                if message.startswith(("开始同步", "结束同步")):
                    event = fields(message)
                    if event.get("event") == "sync_phase_start":
                        phase = event.get("phase", "command")
                    elif event.get("event") == "sync_phase_end":
                        if event.get("phase") == phase:
                            completed_phases.add(phase)
                        phase = "command"
                    continue
                if not message.startswith(("开始 API 请求 ", "结束 API 请求 ")):
                    continue
                event = fields(line)
                try:
                    request_id = int(event["request_id"])
                    endpoint = event["endpoint"]
                    timestamp = int(event["ts_ms"])
                    if event["event"] == "http_request_start":
                        if request_id in seen_ids:
                            flag("duplicate_request_id", line=number, request_id=request_id)
                            continue
                        seen_ids.add(request_id)
                        pending[request_id] = endpoint
                        counts[(phase, endpoint)] += 1
                        timestamps[(phase, endpoint)].append(timestamp)
                        endpoint_stats[endpoint]["timestamps"].append(timestamp)
                    elif event["event"] == "http_request_end":
                        duration = float(event["elapsed_ms"])
                        if not math.isfinite(duration) or duration < 0:
                            raise ValueError
                        if pending.pop(request_id, None) != endpoint:
                            flag("unmatched_request_end", line=number, request_id=request_id)
                            continue
                        status, outcome = event["status"], event["outcome"]
                        stats = endpoint_stats[endpoint]
                        stats["status"][status] += 1
                        stats["outcomes"][outcome] += 1
                        stats["durations"].append(duration)
                        if status == "429" or outcome != "ok":
                            flag("http_429" if status == "429" else "request_failure",
                                 endpoint=endpoint, request_id=request_id, status=status,
                                 outcome=outcome, elapsed_ms=duration)
                    else:
                        raise ValueError
                except (KeyError, ValueError):
                    flag("invalid_request_event", line=number)
    for request_id, endpoint in pending.items():
        flag("missing_request_end", endpoint=endpoint, request_id=request_id)
    if levels["warn"] or levels["error"]:
        flag("diagnostics", warnings=levels["warn"], errors=levels["error"],
             expected_failure=metadata["exit"] == metadata["expected_exit"] != 0)
    limits = list(metadata.get("request_limits", []))
    if (limit := metadata.get("execute_list_limit")) is not None:
        limits.append({"phase": "execute", "endpoint": "/file/list", "limit": limit})
    checked = []
    for limit in limits:
        phase, endpoint = limit["phase"], limit["endpoint"]
        actual = (
            sum(count for (_, label), count in counts.items() if label == endpoint)
            if phase is None
            else counts[(phase, endpoint)]
        )
        result = {**limit, "count": actual}
        if actual > limit["limit"]:
            flag("request_limit", **result)
            status = "exceeded"
        elif not path.is_file() or metadata["exit"] is None:
            status = "unverified"
        elif phase in ("scan", "execute") and phase not in completed_phases:
            flag("missing_budget_phase", **limit)
            status = "unverified"
        else:
            status = "passed"
        checked.append({**result, "status": status})
    return {
        "invocation": invocation,
        **metadata,
        "request_budget_results": checked,
        "requests": [
            {"phase": phase, "endpoint": endpoint, "count": count,
             "peak_requests_in_one_second": peak_per_second(timestamps[(phase, endpoint)])}
            for (phase, endpoint), count in sorted(counts.items())
        ],
    }


def analyze(args):
    invocations = []
    capture_issues = []
    for directory in args.directory.glob("cmd-*"):
        try:
            metadata = json.loads((directory / "invocation.json").read_text(encoding="utf-8"))
        except (OSError, ValueError):
            capture_issues.append(
                {"invocation": directory.name, "kind": "missing_or_invalid_metadata"}
            )
            continue
        invocations.append((directory, metadata))
    if not invocations and not capture_issues:
        raise ValueError("本轮没有已登记的 CLI 调用")
    stats = defaultdict(
        lambda: {"timestamps": [], "durations": [], "status": Counter(), "outcomes": Counter()}
    )
    findings = []
    runs = []
    by_purpose = Counter()
    with (args.directory / "combined.log").open("w", encoding="utf-8") as merged:
        for directory, metadata in sorted(invocations, key=lambda pair: pair[1]["started_ms"]):
            purpose = metadata["purpose"]
            by_purpose[purpose] += 1
            archive_header(directory, metadata, merged)
            if purpose == "case":
                runs.append(read_case(directory, metadata, merged, stats, findings))
            else:
                path = directory / "cli.log"
                if path.is_file():
                    with path.open(encoding="utf-8", errors="replace") as log:
                        shutil.copyfileobj(log, merged)
    endpoints = {}
    for endpoint, data in sorted(stats.items()):
        durations = sorted(data["durations"])
        peak = peak_per_second(data["timestamps"])
        endpoints[endpoint] = {
            "attempts": len(data["timestamps"]),
            "status": data["status"],
            "outcomes": data["outcomes"],
            "p95_ms": durations[math.ceil(len(durations) * 0.95) - 1] if durations else None,
            "max_ms": durations[-1] if durations else None,
            "peak_requests_in_one_second": peak,
        }
        if args.peak_limit is not None and peak > args.peak_limit:
            findings.append({"kind": "peak_limit", "endpoint": endpoint, "count": peak,
                             "limit": args.peak_limit})
    path = args.directory / "analysis.json"
    save_json(path, {
        "scope": "case",
        "capture": {"by_purpose": by_purpose, "issues": capture_issues},
        "invocations": runs,
        "endpoints": endpoints,
        "findings": findings,
    })
    print(f"已归档 {len(invocations)} 次 CLI 调用，分析 {len(runs)} 次用例主命令，"
          f"{len(findings)} 条待核对记录。")
    print(path)
    return 0


def nonnegative(value):
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("数值须为非负整数")
    return parsed


def request_limit(value):
    match = re.fullmatch(r"(?:(scan|execute|command):)?([/a-zA-Z0-9_.-]+)=([0-9]+)", value)
    if match is None:
        raise argparse.ArgumentTypeError("请求预算须为 ENDPOINT=N 或 PHASE:ENDPOINT=N")
    phase, endpoint, limit = match.groups()
    return {"phase": phase, "endpoint": endpoint, "limit": int(limit)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="action", required=True)
    capture = subparsers.add_parser("run")
    capture.add_argument("--directory", type=Path, required=True)
    capture.add_argument("--bin", required=True)
    capture.add_argument("--config", required=True)
    capture.add_argument("--case", required=True)
    capture.add_argument("--purpose", choices=("case", "setup", "poll", "verify", "cleanup"), default="case")
    capture.add_argument("--expected-exit", type=nonnegative, default=0)
    capture.add_argument("--execute-list-limit", type=nonnegative)
    capture.add_argument("--request-limit", action="append", type=request_limit, default=[])
    capture.add_argument("command_args", nargs=argparse.REMAINDER)
    summary = subparsers.add_parser("analyze")
    summary.add_argument("--directory", type=Path, required=True)
    summary.add_argument("--peak-limit", type=nonnegative)
    args = parser.parse_args()
    try:
        return run(args) if args.action == "run" else analyze(args)
    except (OSError, ValueError) as error:
        print(f"请求日志处理失败: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
