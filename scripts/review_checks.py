#!/usr/bin/env python3
"""Run compact, scope-aware code-guideline checks for reviewers and refactors."""

import argparse
from collections.abc import Iterator
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass


ANSI_RE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
RUSTFMT_DIFF_RE = re.compile(r"^Diff in (.+):\d+:$")
WORKSPACE_CARGO_FILES = {
    ".clippy.toml",
    "Cargo.lock",
    "Cargo.toml",
    "clippy.toml",
    "rustfmt.toml",
}
CLIPPY_CONFIG_FILES = {
    ".clippy.toml",
    "Cargo.toml",
    "clippy.toml",
}
JSCPD_SOURCE_SUFFIXES = {
    ".asm",
    ".bash",
    ".c",
    ".cc",
    ".cmake",
    ".cpp",
    ".css",
    ".cxx",
    ".go",
    ".h",
    ".hpp",
    ".java",
    ".js",
    ".jsx",
    ".nix",
    ".py",
    ".rs",
    ".s",
    ".scss",
    ".sh",
    ".ts",
    ".tsx",
    ".zig",
}
JSCPD_SOURCE_NAMES = {"CMakeLists.txt", "Makefile"}
JSCPD_MIN_LINES = 4
JSCPD_MIN_TOKENS = 40
JSCPD_IGNORE = "crates/open62541-sys/open62541/**"
RUST_FILE_LINE_LIMIT = 1000
RUST_USE_START_RE = re.compile(
    r"^(?:pub(?:\([^)]*\))?\s+)?use\s+[A-Za-z0-9_:#*,{}\s]+;?$"
)
RUST_USE_CONTINUATION_RE = re.compile(
    r"^(?:(?:r#)?[A-Za-z_][A-Za-z0-9_]*"
    r"(?:::(?:(?:r#)?[A-Za-z_][A-Za-z0-9_]*|\*))*"
    r"(?:\s+as\s+(?:(?:r#)?[A-Za-z_][A-Za-z0-9_]*|_))?"
    r"[,{};]*|[{},;]+)$"
)
RUST_IMPL_HEADER_RE = re.compile(
    r"^impl(?:<[^{}();=]*>)?\s+[A-Za-z0-9_:#<>'_, ]+\s*\{$"
)
RUST_FN_HEADER_PREFIX_RE = re.compile(
    r"^(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+"
    r"(?:r#)?[A-Za-z_][A-Za-z0-9_]*\s*\($"
)
RUST_MODULE_SCAFFOLD_RE = re.compile(
    r"^(?:#!?\[[^\[\]]*\]"
    r"|(?:pub(?:\([^)]*\))?\s+)?mod\s+(?:r#)?[A-Za-z_][A-Za-z0-9_]*\s*[;{]"
    r"|(?:r#)?[A-Za-z_][A-Za-z0-9_]*!\([^()]*\);)$"
)
RUST_SECTION_DIVIDER_RE = re.compile(
    r"^\s*//(?![/!])\s*(?:[-=─━═]{4,}|"
    r"[-=─━═]{2,}\s+[^-=─━═\s].*?\s+[-=─━═]{2,})\s*$"
)


@dataclass(frozen=True)
class ScopeEntry:
    path: Path
    is_dir: bool


@dataclass(frozen=True)
class Scope:
    label: str
    entries: tuple[ScopeEntry, ...]
    is_all: bool = False

    def contains(self, path: Path) -> bool:
        if self.is_all:
            return True
        return any(
            path == entry.path or (entry.is_dir and path.is_relative_to(entry.path))
            for entry in self.entries
        )

    def command_paths(self, root: Path) -> list[str]:
        if self.is_all:
            return ["."]
        return [entry.path.relative_to(root).as_posix() for entry in self.entries]


@dataclass(frozen=True, order=True)
class Diagnostic:
    path: str
    line: int
    column: int
    level: str
    code: str
    message: str

    def render(self) -> str:
        location = self.path
        if self.line:
            location += f":{self.line}:{self.column}"
        code = f" [{self.code}]" if self.code else ""
        return f"{location}:{code} {self.level}: {self.message}"


@dataclass
class CheckResult:
    name: str
    command: list[str] | None
    status: str
    summary: str
    details: list[str]


@dataclass(frozen=True)
class CargoCheckContext:
    root: Path
    scope: Scope
    packages: tuple[str, ...]
    artifact_dir: Path
    max_items: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Reject ornamental Rust section dividers, warn about oversized Rust "
            "files, and run Clippy, rustfmt check-only, and Rust jscpd with compact "
            "output."
        ),
        epilog=(
            "With no paths, check modified, staged, and untracked files. Pass files or "
            "directories to check any explicit scope, or --all for the full workspace."
        ),
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--all",
        action="store_true",
        help="check the full workspace",
    )
    mode.add_argument(
        "--changed",
        action="store_true",
        help="check Git-changed files explicitly (the default when no paths are given)",
    )
    parser.add_argument(
        "--max-items",
        type=int,
        default=100,
        metavar="N",
        help="maximum items printed per check (default: 100)",
    )
    parser.add_argument(
        "paths",
        nargs="*",
        metavar="PATH",
        help="files or directories to check, including unmodified code",
    )
    args = parser.parse_args()
    if args.max_items < 1:
        parser.error("--max-items must be at least 1")
    if args.paths and (args.all or args.changed):
        parser.error("PATH arguments cannot be combined with --all or --changed")
    return args


def run_capture(
    command: list[str],
    root: Path,
    stdout_path: Path,
    stderr_path: Path,
    env: dict[str, str] | None = None,
) -> int:
    with stdout_path.open("w", encoding="utf-8") as stdout_file, stderr_path.open(
        "w", encoding="utf-8"
    ) as stderr_file:
        try:
            completed = subprocess.run(
                command,
                cwd=root,
                env=env,
                stdout=stdout_file,
                stderr=stderr_file,
                text=True,
                check=False,
            )
        except FileNotFoundError:
            stderr_file.write(f"{command[0]}: command not found\n")
            return 127
    return completed.returncode


def git_output(root: Path, args: list[str]) -> bytes:
    try:
        return subprocess.run(
            ["git", *args],
            cwd=root,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        ).stdout
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise RuntimeError(f"could not determine Git scope: {error}") from error


def repo_root() -> Path:
    try:
        output = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=True,
        ).stdout
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise RuntimeError(f"not inside a Git worktree: {error}") from error
    return Path(output.strip()).resolve()


def changed_scope(root: Path) -> Scope:
    tracked = git_output(
        root,
        ["diff", "HEAD", "--name-only", "-z", "--diff-filter=ACMR"],
    )
    untracked = git_output(
        root,
        ["ls-files", "--others", "--exclude-standard", "-z"],
    )
    names = {
        name.decode("utf-8", errors="surrogateescape")
        for name in (tracked + untracked).split(b"\0")
        if name
    }
    entries = tuple(
        dict.fromkeys(
            ScopeEntry(path=(root / name).resolve(), is_dir=False)
            for name in sorted(names)
            if (root / name).is_file() and not (root / name).is_symlink()
        )
    )
    return Scope(label=f"Git changed ({len(entries)} files)", entries=entries)


def explicit_scope(root: Path, raw_paths: list[str]) -> Scope:
    entries = []
    for raw_path in raw_paths:
        unresolved = Path.cwd() / raw_path
        if unresolved.is_symlink():
            raise ValueError(f"scope path must not be a symbolic link: {raw_path}")
        path = unresolved.resolve()
        if not path.is_relative_to(root):
            raise ValueError(f"scope path is outside the repository: {raw_path}")
        if not path.exists():
            raise ValueError(f"scope path does not exist: {raw_path}")
        entries.append(ScopeEntry(path=path, is_dir=path.is_dir()))
    unique_entries = tuple(dict.fromkeys(entries))
    unique = tuple(
        entry
        for entry in unique_entries
        if not any(
            parent != entry
            and parent.is_dir
            and entry.path.is_relative_to(parent.path)
            for parent in unique_entries
        )
    )
    return Scope(label=f"explicit ({len(unique)} paths)", entries=unique)


def select_scope(args: argparse.Namespace, root: Path) -> Scope:
    if args.all:
        return Scope(
            label="full workspace",
            entries=(ScopeEntry(path=root, is_dir=True),),
            is_all=True,
        )
    if args.paths:
        return explicit_scope(root, args.paths)
    return changed_scope(root)


def metadata_command(root: Path) -> list[str]:
    del root  # the command is independent of the working directory
    return ["cargo", "metadata", "--format-version=1", "--no-deps"]


def load_packages(root: Path) -> list[tuple[str, Path]]:
    command = metadata_command(root)
    try:
        completed = subprocess.run(
            command,
            cwd=root,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=True,
        )
    except FileNotFoundError as error:
        raise RuntimeError(f"{command[0]}: command not found") from error
    except subprocess.CalledProcessError as error:
        reason = error.stderr.strip() or str(error)
        raise RuntimeError(f"cargo metadata failed: {reason}") from error
    metadata = json.loads(completed.stdout)
    return sorted(
        (package["name"], Path(package["manifest_path"]).resolve().parent)
        for package in metadata["packages"]
    )


def packages_for_scope(
    root: Path, packages: list[tuple[str, Path]], scope: Scope
) -> list[str]:
    if scope.is_all:
        return [name for name, _ in packages]
    if any(
        entry.path.parent == root and entry.path.name in WORKSPACE_CARGO_FILES
        or entry.path.is_relative_to(root / ".cargo")
        for entry in scope.entries
    ):
        return [name for name, _ in packages]

    selected = []
    for name, package_root in packages:
        for entry in scope.entries:
            if entry.is_dir:
                intersects = entry.path.is_relative_to(
                    package_root
                ) or package_root.is_relative_to(entry.path)
            else:
                intersects = entry.path.is_relative_to(package_root)
            if intersects:
                selected.append(name)
                break
    return selected


def normalize_diagnostic_path(root: Path, filename: str) -> Path | None:
    if not filename or filename.startswith("<"):
        return None
    path = Path(filename)
    if not path.is_absolute():
        path = root / path
    return path.resolve()


def display_path(root: Path, path: Path) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.as_posix()


def scoped_rust_files(root: Path, scope: Scope) -> list[Path]:
    listed = git_output(
        root,
        ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
    )
    listed_paths = {
        root / name.decode("utf-8", errors="surrogateescape")
        for name in listed.split(b"\0")
        if name
    }
    paths = {
        path.resolve()
        for path in listed_paths
        if path.is_file() and not path.is_symlink()
    }
    paths.update(
        entry.path
        for entry in scope.entries
        if not entry.is_dir and entry.path.suffix == ".rs"
    )
    return sorted(
        path
        for path in paths
        if path.is_relative_to(root)
        and path.suffix == ".rs"
        and path.is_file()
        and scope.contains(path)
    )


def rust_char_literal_end(line: str, start: int) -> int | None:
    content = start + 1
    if content >= len(line):
        return None
    if line[content] != "\\":
        end = content + 1
    elif line.startswith("\\u{", content):
        brace = line.find("}", content + 3)
        if brace == -1:
            return None
        end = brace + 1
    elif line.startswith("\\x", content):
        end = content + 4
    else:
        end = content + 2
    if end < len(line) and line[end] == "'":
        return end + 1
    return None


def rust_raw_string_start(line: str, start: int) -> tuple[str, int] | None:
    for prefix in ("br", "cr", "r"):
        if not line.startswith(prefix, start):
            continue
        marker = start + len(prefix)
        while marker < len(line) and line[marker] == "#":
            marker += 1
        if marker < len(line) and line[marker] == '"':
            hashes = marker - start - len(prefix)
            return '"' + "#" * hashes, marker + 1
    return None


def rust_line_comments(source: str) -> Iterator[tuple[int, str]]:
    block_comment_depth = 0
    in_string = False
    raw_string_terminator = None

    for line_number, line in enumerate(source.splitlines(), start=1):
        index = 0
        while index < len(line):
            if raw_string_terminator is not None:
                terminator = line.find(raw_string_terminator, index)
                if terminator == -1:
                    break
                index = terminator + len(raw_string_terminator)
                raw_string_terminator = None
                continue

            if in_string:
                if line[index] == "\\":
                    index += 2
                elif line[index] == '"':
                    in_string = False
                    index += 1
                else:
                    index += 1
                continue

            if block_comment_depth:
                if line.startswith("/*", index):
                    block_comment_depth += 1
                    index += 2
                elif line.startswith("*/", index):
                    block_comment_depth -= 1
                    index += 2
                else:
                    index += 1
                continue

            if line.startswith("//", index):
                if index == 0 or line[:index].isspace():
                    yield line_number, line
                break
            if line.startswith("/*", index):
                block_comment_depth = 1
                index += 2
                continue

            raw_string = rust_raw_string_start(line, index)
            if raw_string is not None:
                raw_string_terminator, index = raw_string
                continue
            if line[index] == '"':
                in_string = True
                index += 1
                continue
            if line[index] == "'":
                char_end = rust_char_literal_end(line, index)
                if char_end is not None:
                    index = char_end
                    continue
            index += 1


def run_file_size_check(
    root: Path,
    scope: Scope,
    max_items: int,
) -> CheckResult:
    rust_files = scoped_rust_files(root, scope)
    if not rust_files:
        return CheckResult(
            "file-size", None, "SKIP", "no Rust source files in scope", []
        )

    oversized = []
    for path in rust_files:
        with path.open("rb") as source:
            line_count = sum(1 for _ in source)
        if line_count > RUST_FILE_LINE_LIMIT:
            oversized.append(f"{display_path(root, path)}: {line_count:,} lines")

    if not oversized:
        return CheckResult(
            "file-size",
            None,
            "PASS",
            f"no scoped Rust files exceed {RUST_FILE_LINE_LIMIT:,} lines",
            [],
        )
    visible = oversized[:max_items]
    if len(oversized) > max_items:
        visible.append(f"... {len(oversized) - max_items} more oversized files")
    count = len(oversized)
    noun = "file" if count == 1 else "files"
    verb = "exceeds" if count == 1 else "exceed"
    return CheckResult(
        "file-size",
        None,
        "WARN",
        f"{count} scoped Rust {noun} {verb} {RUST_FILE_LINE_LIMIT:,} lines",
        visible,
    )


def run_section_divider_check(
    root: Path,
    scope: Scope,
    max_items: int,
) -> CheckResult:
    rust_files = scoped_rust_files(root, scope)
    if not rust_files:
        return CheckResult(
            "section-dividers", None, "SKIP", "no Rust source files in scope", []
        )

    violations = []
    for path in rust_files:
        source = path.read_text(encoding="utf-8", errors="replace")
        violations.extend(
            f"{display_path(root, path)}:{line_number}"
            for line_number, line in rust_line_comments(source)
            if RUST_SECTION_DIVIDER_RE.fullmatch(line)
        )

    if not violations:
        return CheckResult(
            "section-dividers",
            None,
            "PASS",
            "no ornamental section-divider comments in scoped Rust files",
            [],
        )

    visible = violations[:max_items]
    if len(violations) > max_items:
        remaining = len(violations) - max_items
        visible.append(f"... {remaining} more section-divider comments")
    count = len(violations)
    noun = "comment" if count == 1 else "comments"
    return CheckResult(
        "section-dividers",
        None,
        "FAIL",
        f"{count} ornamental section-divider {noun} in scoped Rust files",
        visible,
    )


def parse_clippy(
    root: Path, scope: Scope, stdout_path: Path
) -> tuple[list[Diagnostic], int, int]:
    diagnostics = set()
    suppressed = 0
    malformed = 0
    for line in stdout_path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            if line.strip():
                malformed += 1
            continue
        if record.get("reason") != "compiler-message":
            continue
        message = record.get("message", {})
        level = message.get("level", "")
        if level not in {"warning", "error"}:
            continue
        spans = [span for span in message.get("spans", []) if span.get("is_primary")]
        scoped_spans = []
        for span in spans:
            path = normalize_diagnostic_path(root, span.get("file_name", ""))
            if path is not None and scope.contains(path):
                scoped_spans.append((span, path))
        if level == "warning" and not scoped_spans:
            suppressed += 1
            continue
        display_spans = scoped_spans
        if not display_spans:
            for span in spans:
                path = normalize_diagnostic_path(root, span.get("file_name", ""))
                if path is not None:
                    display_spans.append((span, path))
                    break
        if display_spans:
            span, path = display_spans[0]
            diagnostic_path = display_path(root, path)
            line_number = int(span.get("line_start", 0))
            column = int(span.get("column_start", 0))
        else:
            diagnostic_path = "<workspace>"
            line_number = 0
            column = 0
        code_record = message.get("code") or {}
        diagnostics.add(
            Diagnostic(
                path=diagnostic_path,
                line=line_number,
                column=column,
                level=level,
                code=code_record.get("code", ""),
                message=" ".join(message.get("message", "").split()),
            )
        )
    return sorted(diagnostics), suppressed, malformed


def clippy_diagnostic_scope(root: Path, scope: Scope) -> Scope:
    if scope.is_all:
        return scope

    entries = list(scope.entries)
    for entry in scope.entries:
        if entry.path.name in CLIPPY_CONFIG_FILES:
            entries.append(ScopeEntry(entry.path.parent, True))
        elif entry.path.is_relative_to(root / ".cargo"):
            entries.append(ScopeEntry(root, True))

    return Scope(
        label=scope.label,
        entries=tuple(dict.fromkeys(entries)),
    )


def truncate(items: list[str], maximum: int) -> list[str]:
    visible = items[:maximum]
    if len(items) > maximum:
        visible.append(f"... {len(items) - maximum} more (see raw artifacts)")
    return visible


def run_clippy(context: CargoCheckContext) -> CheckResult:
    if not context.packages:
        return CheckResult("clippy", None, "SKIP", "no Cargo packages in scope", [])
    command = [
        "cargo",
        "clippy",
        "--no-deps",
        "--all-targets",
        "--all-features",
        "--quiet",
        "--message-format=json",
        "--color=never",
    ]
    if context.scope.is_all:
        command.append("--workspace")
    else:
        for package in context.packages:
            command.extend(["--package", package])
    stdout_path = context.artifact_dir / "clippy.jsonl"
    stderr_path = context.artifact_dir / "clippy.stderr"
    env = {**os.environ, "CLIPPY_DISABLE_DOCS_LINKS": "1"}
    return_code = run_capture(command, context.root, stdout_path, stderr_path, env)
    diagnostics, suppressed, malformed = parse_clippy(
        context.root,
        clippy_diagnostic_scope(context.root, context.scope),
        stdout_path,
    )
    details = [diagnostic.render() for diagnostic in diagnostics]
    stderr = stderr_path.read_text(encoding="utf-8", errors="replace").strip()
    if return_code != 0 and not any(item.level == "error" for item in diagnostics):
        details.extend(stderr.splitlines()[:10])
        return CheckResult(
            "clippy",
            command,
            "ERROR",
            f"command exited {return_code} without a parsed compiler error",
            truncate(details, context.max_items),
        )
    status = "FAIL" if diagnostics else "PASS"
    summary = f"{len(diagnostics)} scoped diagnostics"
    if suppressed:
        summary += f", {suppressed} out-of-scope warnings suppressed"
    if malformed:
        summary += f", {malformed} non-JSON stdout lines captured"
    return CheckResult(
        "clippy", command, status, summary, truncate(details, context.max_items)
    )


def rustfmt_command(root: Path) -> list[str]:
    del root  # the command is independent of the working directory
    try:
        version = subprocess.run(
            ["rustc", "--version"],
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            check=True,
        ).stdout
    except (FileNotFoundError, subprocess.CalledProcessError):
        version = ""
    if "nightly" in version:
        return ["cargo", "fmt"]
    if shutil.which("rustup"):
        return ["cargo", "+nightly", "fmt"]
    raise RuntimeError("rustfmt requires a nightly toolchain, but none was found")


def parse_rustfmt_paths(root: Path, output: str) -> tuple[list[Path], list[str]]:
    paths = []
    other = []
    for raw_line in output.splitlines():
        line = ANSI_RE.sub("", raw_line).strip()
        if not line:
            continue
        diff_match = RUSTFMT_DIFF_RE.match(line)
        candidate = Path(diff_match.group(1) if diff_match else line)
        if not candidate.is_absolute():
            candidate = root / candidate
        candidate = candidate.resolve()
        if candidate.is_relative_to(root) and candidate.suffix == ".rs":
            paths.append(candidate)
        else:
            other.append(line)
    return paths, other


def run_rustfmt(context: CargoCheckContext) -> CheckResult:
    if not context.packages:
        return CheckResult("rustfmt", None, "SKIP", "no Cargo packages in scope", [])
    command = [*rustfmt_command(context.root), "--check", "--message-format=short"]
    if context.scope.is_all:
        command.append("--all")
    else:
        for package in context.packages:
            command.extend(["--package", package])
    stdout_path = context.artifact_dir / "rustfmt.stdout"
    stderr_path = context.artifact_dir / "rustfmt.stderr"
    return_code = run_capture(command, context.root, stdout_path, stderr_path)
    stdout = stdout_path.read_text(encoding="utf-8", errors="replace")
    stderr = stderr_path.read_text(encoding="utf-8", errors="replace")
    output = "\n".join([stdout, stderr])
    paths, other = parse_rustfmt_paths(context.root, output)
    scoped = sorted({path for path in paths if context.scope.contains(path)})
    suppressed = len(set(paths) - set(scoped))
    if scoped:
        details = [path.relative_to(context.root).as_posix() for path in scoped]
        summary = f"{len(scoped)} files need formatting"
        if suppressed:
            summary += f", {suppressed} out-of-scope files suppressed"
        return CheckResult(
            "rustfmt",
            command,
            "FAIL",
            summary,
            truncate(details, context.max_items),
        )
    if return_code != 0 and not paths:
        return CheckResult(
            "rustfmt",
            command,
            "ERROR",
            f"command exited {return_code}",
            truncate(other, context.max_items),
        )
    summary = "scope is formatted"
    if suppressed:
        summary += f", {suppressed} out-of-scope files suppressed"
    return CheckResult("rustfmt", command, "PASS", summary, [])


def is_jscpd_source(path: Path) -> bool:
    return path.suffix.lower() in JSCPD_SOURCE_SUFFIXES or path.name in JSCPD_SOURCE_NAMES


def jscpd_scan_paths(root: Path, scope: Scope) -> list[str]:
    if scope.is_all:
        return ["."]
    directories = {
        entry.path if entry.is_dir else entry.path.parent
        for entry in scope.entries
        if entry.is_dir or is_jscpd_source(entry.path)
    }
    minimal = sorted(
        directory
        for directory in directories
        if not any(
            parent != directory and directory.is_relative_to(parent)
            for parent in directories
        )
    )
    return [directory.relative_to(root).as_posix() or "." for directory in minimal]


def rust_clone_starts_inside_use(file: dict) -> bool:
    """Verify that a clone's first source line is inside a multiline `use`."""
    try:
        lines = Path(file["name"]).read_text(encoding="utf-8").splitlines()
        start_index = int(file["start"]) - 1
        line = lines[start_index].strip()
    except (OSError, IndexError, KeyError, TypeError, ValueError):
        return False

    if not RUST_USE_CONTINUATION_RE.fullmatch(line):
        return False
    for previous in reversed(lines[:start_index]):
        line = previous.strip()
        if RUST_USE_START_RE.fullmatch(line):
            return not line.endswith(";")
        if not RUST_USE_CONTINUATION_RE.fullmatch(line):
            return False
    return False


def is_rust_use_preamble_clone(duplicate: dict) -> bool:
    """Return whether imports are required for a Rust clone to meet the gate.

    Every Rust module opens with the same shape: module declarations, a
    `#[cfg(test)]` attribute, a macro invocation that implements a trait, and
    the import block. Such a clone reports the language, not duplicated logic,
    and a merge of two import blocks costs more than it saves. The accepted
    scaffolding stays below JSCPD_MIN_LINES lines, so a clone that holds that
    many lines outside the import block remains a finding.
    """
    if duplicate.get("format") != "rust":
        return False
    fragment = duplicate.get("fragment", "")
    lines = fragment.splitlines()
    saw_use = False
    in_use = False
    in_impl_header = False
    first_significant_line = True
    has_partial_use_prefix = False
    first_use_line = None
    item_header_line = None
    scaffold_lines = 0

    for line_index, raw_line in enumerate(lines):
        line = raw_line.strip()
        if not line:
            continue
        if RUST_USE_START_RE.fullmatch(line):
            if in_impl_header:
                return False
            saw_use = True
            if first_use_line is None:
                first_use_line = line_index
            in_use = not line.endswith(";")
        elif in_use and RUST_USE_CONTINUATION_RE.fullmatch(line):
            in_use = not line.endswith(";")
        elif (
            first_significant_line
            and RUST_USE_CONTINUATION_RE.fullmatch(line)
            and (line.endswith(",") or line in {"}", "};"})
        ):
            has_partial_use_prefix = True
            in_use = not line.endswith(";")
        elif saw_use and RUST_IMPL_HEADER_RE.fullmatch(line):
            in_impl_header = True
            item_header_line = line_index
        elif in_impl_header and RUST_FN_HEADER_PREFIX_RE.fullmatch(line):
            pass
        elif not saw_use and RUST_MODULE_SCAFFOLD_RE.fullmatch(line):
            scaffold_lines += 1
        else:
            return False
        first_significant_line = False

    if not saw_use:
        return False

    non_use_lines = scaffold_lines
    if has_partial_use_prefix and not all(
        rust_clone_starts_inside_use(duplicate[file_key])
        for file_key in ("firstFile", "secondFile")
    ):
        non_use_lines += first_use_line or 0
    if item_header_line is not None:
        non_use_lines += len(lines) - item_header_line

    return non_use_lines < JSCPD_MIN_LINES


def parse_jscpd_report(
    root: Path, scope: Scope, report_path: Path
) -> tuple[list[str], int]:
    report = json.loads(report_path.read_text(encoding="utf-8"))
    details = []
    suppressed = 0
    for duplicate in report.get("duplicates", []):
        if is_rust_use_preamble_clone(duplicate):
            suppressed += 1
            continue
        first = duplicate["firstFile"]
        second = duplicate["secondFile"]
        first_path = Path(first["name"]).resolve()
        second_path = Path(second["name"]).resolve()
        if not (scope.contains(first_path) or scope.contains(second_path)):
            suppressed += 1
            continue
        first_location = (
            f"{display_path(root, first_path)}:{first['start']}-{first['end']}"
        )
        if first_path == second_path:
            second_location = f"{second['start']}-{second['end']}"
        else:
            second_location = (
                f"{display_path(root, second_path)}:{second['start']}-{second['end']}"
            )
        details.append(f"{first_location} ~ {second_location}")
    return details, suppressed


def run_jscpd(
    root: Path,
    scope: Scope,
    artifact_dir: Path,
    max_items: int,
) -> CheckResult:
    paths = jscpd_scan_paths(root, scope)
    if not paths:
        return CheckResult(
            "jscpd", None, "SKIP", "no supported source files in scope", []
        )
    report_dir = artifact_dir / "jscpd-report"
    command = [
        "jscpd",
        "--reporters",
        "json",
        "--absolute",
        "--no-tips",
        "--min-lines",
        str(JSCPD_MIN_LINES),
        "--min-tokens",
        str(JSCPD_MIN_TOKENS),
        "--ignore",
        JSCPD_IGNORE,
        "--output",
        str(report_dir),
        *paths,
    ]
    stdout_path = artifact_dir / "jscpd.stdout"
    stderr_path = artifact_dir / "jscpd.stderr"
    return_code = run_capture(command, root, stdout_path, stderr_path)
    report_path = report_dir / "jscpd-report.json"
    if not report_path.exists():
        output = "\n".join(
            [
                stdout_path.read_text(encoding="utf-8", errors="replace"),
                stderr_path.read_text(encoding="utf-8", errors="replace"),
            ]
        )
        details = [line for line in output.splitlines() if line.strip()]
        return CheckResult(
            "jscpd",
            command,
            "ERROR",
            f"command exited {return_code} without producing a JSON report",
            truncate(details, max_items),
        )
    clone_lines, suppressed = parse_jscpd_report(root, scope, report_path)
    if return_code != 0 and not clone_lines:
        return CheckResult(
            "jscpd",
            command,
            "ERROR",
            f"command exited {return_code}",
            [],
        )
    status = "FAIL" if clone_lines else "PASS"
    summary = f"{len(clone_lines)} scoped clone pairs"
    if suppressed:
        summary += f", {suppressed} pairs suppressed"
    return CheckResult(
        "jscpd",
        command,
        status,
        summary,
        truncate(clone_lines, max_items),
    )


def compact_command(command: list[str]) -> str:
    rendered = shlex.join(command)
    if len(rendered) <= 240:
        return rendered
    if command[0] == "jscpd":
        scan_start = command.index("--output") + 2
        prefix = shlex.join(command[:scan_start])
        return f"{prefix} <{len(command) - scan_start} scan roots>"
    if command[:2] in (["cargo", "clippy"], ["cargo", "fmt"]):
        try:
            first_package = command.index("--package")
        except ValueError:
            first_package = len(command)
        if first_package < len(command):
            package_count = command[first_package:].count("--package")
            return (
                f"{shlex.join(command[:first_package])} "
                f"--package <{package_count} packages>"
            )
    return f"{shlex.quote(command[0])} <{len(command) - 1} arguments>"


def render(scope: Scope, results: list[CheckResult], artifact_dir: Path) -> int:
    print(f"scope: {scope.label}")
    print(f"invocation: {shlex.join([Path(sys.argv[0]).as_posix(), *sys.argv[1:]])}")
    commands_path = artifact_dir / "commands.txt"
    commands_path.write_text(
        "\n".join(
            shlex.join(result.command)
            for result in results
            if result.command is not None
        )
        + "\n",
        encoding="utf-8",
    )
    for result in results:
        print(f"{result.name}: {result.status} ({result.summary})")
        if result.command:
            print(f"  command: {compact_command(result.command)}")
        for detail in result.details:
            print(f"  {detail}")
    has_error = any(result.status == "ERROR" for result in results)
    has_failure = any(result.status == "FAIL" for result in results)
    has_compacted_command = any(
        result.command is not None
        and compact_command(result.command) != shlex.join(result.command)
        for result in results
    )
    if has_error or has_failure:
        print(f"raw artifacts: {artifact_dir}")
    elif has_compacted_command:
        print(f"full commands: {commands_path}")
    else:
        shutil.rmtree(artifact_dir)
    if has_error:
        return 2
    if has_failure:
        return 1
    return 0


def run_available_checks(
    root: Path,
    scope: Scope,
    packages: list[str],
    artifact_dir: Path,
    max_items: int,
) -> list[CheckResult]:
    results = []
    context = CargoCheckContext(
        root=root,
        scope=scope,
        packages=tuple(packages),
        artifact_dir=artifact_dir,
        max_items=max_items,
    )
    for name, check in (("clippy", run_clippy), ("rustfmt", run_rustfmt)):
        try:
            result = check(context)
        except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
            result = CheckResult(name, None, "ERROR", str(error), [])
        results.append(result)
    try:
        jscpd = run_jscpd(root, scope, artifact_dir, max_items)
    except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
        jscpd = CheckResult("jscpd", None, "ERROR", str(error), [])
    results.append(jscpd)
    return results


def main() -> int:
    args = parse_args()
    try:
        root = repo_root()
        scope = select_scope(args, root)
        artifact_dir = Path(tempfile.mkdtemp(prefix="rgdsd-review-checks-"))
        try:
            file_size = run_file_size_check(root, scope, args.max_items)
        except (OSError, RuntimeError, ValueError) as error:
            file_size = CheckResult("file-size", None, "ERROR", str(error), [])
        try:
            section_dividers = run_section_divider_check(root, scope, args.max_items)
        except (OSError, RuntimeError, ValueError) as error:
            section_dividers = CheckResult(
                "section-dividers", None, "ERROR", str(error), []
            )
        try:
            packages = packages_for_scope(
                root,
                load_packages(root),
                scope,
            )
            package_error = None
        except (RuntimeError, json.JSONDecodeError) as error:
            packages = []
            package_error = error

        if package_error is None:
            results = [
                file_size,
                section_dividers,
                *run_available_checks(
                    root,
                    scope,
                    packages,
                    artifact_dir,
                    args.max_items,
                ),
            ]
        else:
            failed_metadata_command = metadata_command(root)
            clippy = CheckResult(
                "clippy", failed_metadata_command, "ERROR", str(package_error), []
            )
            rustfmt = CheckResult(
                "rustfmt", failed_metadata_command, "ERROR", str(package_error), []
            )
            try:
                jscpd = run_jscpd(root, scope, artifact_dir, args.max_items)
            except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
                jscpd = CheckResult("jscpd", None, "ERROR", str(error), [])
            results = [file_size, section_dividers, clippy, rustfmt, jscpd]
        return render(scope, results, artifact_dir)
    except (RuntimeError, ValueError, json.JSONDecodeError) as error:
        print(f"review-checks: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
