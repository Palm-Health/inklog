#!/usr/bin/env python3
"""Reject documentation or source drift around Inklog's mandatory masking boundary."""

from __future__ import annotations

import argparse
import re
import shutil
import sys
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Callable


DOCUMENTS = (
    Path("README.md"),
    Path("README_zh.md"),
    Path("docs/USER_GUIDE.md"),
    Path("docs/SECURITY.md"),
    Path("docs/API_REFERENCE.md"),
)

REQUIRED_CLAIMS = {
    Path("README.md"): "`masking_enabled` compatibility field is a no-op",
    Path("README_zh.md"): "`masking_enabled` 兼容字段为无操作字段",
    Path("docs/USER_GUIDE.md"): "`masking_enabled` 是已弃用的无操作兼容字段",
    Path("docs/SECURITY.md"): "masking_enabled = true  # 已弃用的无操作兼容字段",
    Path("docs/API_REFERENCE.md"): "`masking_enabled` | `bool` | `true` | 已弃用的无操作兼容字段",
    Path("src/config.rs"): "Deprecated no-op compatibility field",
}

LEGACY_ENV = re.compile(r"(?<!GLOBAL_)\bINKLOG_MASKING_ENABLED\b")
GLOBAL_FIELD = r"(?:[A-Za-z_][A-Za-z0-9_]*\s*\.\s*)+global\s*\.\s*masking_enabled"
GLOBAL_CONDITIONAL = re.compile(
    rf"\b(?:if|while|match)\b[^\n{{}};]{{0,240}}\b{GLOBAL_FIELD}\b"
)
ACTION_CLAIM = re.compile(
    r"(?:enable(?:s|d)?|disable(?:s|d)?|controls?|conditional|"
    r"启用|禁用|控制|自动脱敏|默认启用)",
    re.IGNORECASE,
)
NEGATED_ACTION = re.compile(
    r"(?:\b(?:does not|cannot|must not|never)\s+"
    r"(?:enable|disable|control|gate)(?:\s+or\s+(?:enable|disable|control|gate))*\b|"
    r"\bnot\s+conditional\b|(?:不|不能|不会|从不)(?:启用|禁用|控制)"
    r"(?:或(?:启用|禁用|控制))*)",
    re.IGNORECASE,
)
POSITIVE_ACTION = re.compile(
    r"(?:\benable(?:s|d)?\b|\bdisable(?:s|d)?\b|\bcontrols?\b|"
    r"\bgates?\b|\bconditional\b|启用|禁用|控制|自动脱敏|默认启用)",
    re.IGNORECASE,
)
NO_OP_CLAIM = re.compile(
    r"(?:\bno[- ]?op\b|\binert\b|\bignored\b|\bnot consulted\b|"
    r"\bhas no effect\b|\bdoes not (?:enable|disable|control|gate)\b|"
    r"\bcannot (?:enable|disable|control|gate)\b|"
    r"无操作|不启用|不禁用|不控制|不参与|不影响|未被使用)",
    re.IGNORECASE,
)
GLOBAL_CONTEXT = re.compile(
    r"(?:GlobalConfig|global configuration|\bglobal\s*:|\[global\]|全局)", re.IGNORECASE
)
SINK_CONTEXT = re.compile(
    r"(?:ConsoleSinkConfig|FileSinkConfig|ConsoleSink|FileSink|\[console_sink\]|\[file_sink\]|"
    r"additional sink|redundant .*pass|额外.*Sink|Sink.*额外)",
    re.IGNORECASE,
)
RUST_COMMENT = re.compile(r"^\s*(?://[/!]?|/\*+|\*+/?)")


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def relative(path: Path, root: Path) -> Path:
    return path.relative_to(root)


def adjacent_claim(lines: list[str], index: int, rust_source: bool) -> str:
    parts: list[str] = []
    for line in lines[index : index + 4]:
        if rust_source:
            if not RUST_COMMENT.match(line):
                break
            normalized = RUST_COMMENT.sub("", line, count=1).strip()
        else:
            normalized = line.strip()
        if not normalized:
            break
        parts.append(normalized)
    return " ".join(parts)


def has_positive_action(claim: str) -> bool:
    return POSITIVE_ACTION.search(NEGATED_ACTION.sub("", claim)) is not None


def check(root: Path = Path(".")) -> list[str]:
    root = root.resolve()
    errors: list[str] = []
    document_paths = tuple(root / path for path in DOCUMENTS)
    source_paths = tuple((root / "src").rglob("*.rs"))

    for path in document_paths + source_paths:
        rel = relative(path, root)
        text = path.read_text(encoding="utf-8")

        for match in LEGACY_ENV.finditer(text):
            errors.append(
                f"{rel}:{line_number(text, match.start())}: use INKLOG_GLOBAL_MASKING_ENABLED"
            )

        for match in GLOBAL_CONDITIONAL.finditer(text):
            errors.append(
                f"{rel}:{line_number(text, match.start())}: global masking compatibility field must not gate masking"
            )

        lines = text.splitlines()
        for index, line in enumerate(lines):
            if "masking_enabled" not in line:
                continue
            rust_source = path in source_paths
            if rust_source and not RUST_COMMENT.match(line):
                continue

            claim = adjacent_claim(lines, index, rust_source)
            claim_text = claim.replace("masking_enabled", "")
            if not ACTION_CLAIM.search(claim_text):
                continue

            scope = "\n".join(lines[max(0, index - 120) : min(len(lines), index + 4)])
            global_matches = tuple(GLOBAL_CONTEXT.finditer(scope))
            sink_matches = tuple(SINK_CONTEXT.finditer(scope))
            global_position = global_matches[-1].start() if global_matches else -1
            sink_position = sink_matches[-1].start() if sink_matches else -1
            is_sink_claim = sink_position > global_position or rel in (
                Path("src/sink/console.rs"),
                Path("src/sink/file.rs"),
            )
            is_global_claim = global_position > sink_position
            is_global_or_ambiguous = is_global_claim or not is_sink_claim
            lacks_affirmative_no_op = not NO_OP_CLAIM.search(claim)
            if is_global_or_ambiguous and (
                has_positive_action(claim_text) or lacks_affirmative_no_op
            ):
                errors.append(
                    f"{rel}:{index + 1}: global masking action claim must state deprecated/no-op semantics"
                )

    for path, claim in REQUIRED_CLAIMS.items():
        text = (root / path).read_text(encoding="utf-8")
        if claim not in text:
            errors.append(f"{path}: required mandatory-masking contract text is missing")

    return errors


def copy_fixture(root: Path, destination: Path) -> None:
    for path in DOCUMENTS:
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / path, target)
    shutil.copytree(root / "src", destination / "src")


def append(path: Path, text: str) -> None:
    with path.open("a", encoding="utf-8") as stream:
        stream.write(f"\n{text}\n")


def run_probe(
    root: Path,
    temporary_root: Path,
    name: str,
    mutation: Callable[[Path], None],
    expected_fragment: str | None,
) -> None:
    fixture = temporary_root / name
    copy_fixture(root, fixture)
    mutation(fixture)
    violations = check(fixture)
    if expected_fragment is None:
        if violations:
            raise AssertionError(f"safe control {name} failed: {'; '.join(violations)}")
        return
    if not any(expected_fragment in violation for violation in violations):
        raise AssertionError(f"adversarial probe {name} was accepted: {'; '.join(violations)}")


def self_test(root: Path = Path(".")) -> None:
    root = root.resolve()
    with TemporaryDirectory(prefix="inklog-masking-contract-") as temp:
        temporary_root = Path(temp)
        run_probe(
            root,
            temporary_root,
            "arbitrary_identifier_condition",
            lambda fixture: append(
                fixture / "src/manager.rs",
                "fn masking_probe(cfg: ProbeConfig) { if cfg.global.masking_enabled { disable_mandatory_masking(); } }",
            ),
            "must not gate masking",
        )
        run_probe(
            root,
            temporary_root,
            "rustdoc_enablement_claim",
            lambda fixture: append(
                fixture / "src/config.rs",
                "/// GlobalConfig masking_enabled enables mandatory masking.",
            ),
            "must state deprecated/no-op semantics",
        )
        run_probe(
            root,
            temporary_root,
            "compatibility_contradiction",
            lambda fixture: append(
                fixture / "src/config.rs",
                "/// GlobalConfig masking_enabled enables mandatory masking for compatibility.",
            ),
            "must state deprecated/no-op semantics",
        )
        run_probe(
            root,
            temporary_root,
            "deprecated_contradiction",
            lambda fixture: append(
                fixture / "src/config.rs",
                "/// GlobalConfig masking_enabled controls mandatory masking but is deprecated.",
            ),
            "must state deprecated/no-op semantics",
        )
        run_probe(
            root,
            temporary_root,
            "no_op_contradiction",
            lambda fixture: append(
                fixture / "src/config.rs",
                "/// GlobalConfig masking_enabled is a no-op but enables mandatory masking.",
            ),
            "must state deprecated/no-op semantics",
        )
        run_probe(
            root,
            temporary_root,
            "markdown_multiline_contradiction",
            lambda fixture: append(
                fixture / "README.md",
                "GlobalConfig masking_enabled\nenables mandatory masking.",
            ),
            "must state deprecated/no-op semantics",
        )
        run_probe(
            root,
            temporary_root,
            "rustdoc_multiline_contradiction",
            lambda fixture: append(
                fixture / "src/config.rs",
                "/// GlobalConfig masking_enabled\n/// enables mandatory masking.",
            ),
            "must state deprecated/no-op semantics",
        )
        run_probe(
            root,
            temporary_root,
            "sink_level_control",
            lambda fixture: append(
                fixture / "src/sink/console.rs",
                "/// ConsoleSinkConfig masking_enabled enables an additional sink pass.",
            ),
            None,
        )
        run_probe(
            root,
            temporary_root,
            "global_no_op_control",
            lambda fixture: append(
                fixture / "src/config.rs",
                "/// GlobalConfig masking_enabled is a deprecated no-op compatibility field; adapters always mask.",
            ),
            None,
        )
        run_probe(
            root,
            temporary_root,
            "global_no_effect_control",
            lambda fixture: append(
                fixture / "src/config.rs",
                "/// GlobalConfig masking_enabled does not control mandatory masking and has no effect; it is retained for compatibility.",
            ),
            None,
        )
        run_probe(
            root,
            temporary_root,
            "markdown_multiline_no_op_control",
            lambda fixture: append(
                fixture / "README.md",
                "GlobalConfig masking_enabled\ndoes not control mandatory masking; it is a no-op compatibility field.",
            ),
            None,
        )
        run_probe(
            root,
            temporary_root,
            "rustdoc_multiline_no_op_control",
            lambda fixture: append(
                fixture / "src/config.rs",
                "/// GlobalConfig masking_enabled\n/// does not control mandatory masking; it is a no-op compatibility field.",
            ),
            None,
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run adversarial temp-copy probes after checking the current tree",
    )
    args = parser.parse_args()

    violations = check()
    if violations:
        print("\n".join(violations), file=sys.stderr)
        return 1
    if args.self_test:
        self_test()
        print("mandatory masking guard adversarial probes: ok")
    print("mandatory masking documentation and source contract: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
