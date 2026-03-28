import ast
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
EXCLUDED_PARTS = {
    ".git",
    ".venv",
    "__pycache__",
    "generated",
}


def python_sources():
    for path in REPO_ROOT.rglob("*.py"):
        if any(part in EXCLUDED_PARTS for part in path.parts):
            continue
        yield path


def is_sys_path(node: ast.AST) -> bool:
    return (
        isinstance(node, ast.Attribute)
        and node.attr == "path"
        and isinstance(node.value, ast.Name)
        and node.value.id == "sys"
    )


def test_repo_python_never_mutates_sys_path():
    offenders: list[str] = []

    for path in python_sources():
        tree = ast.parse(path.read_text(), filename=str(path))
        for node in ast.walk(tree):
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute):
                if is_sys_path(node.func.value) and node.func.attr in {
                    "append",
                    "extend",
                    "insert",
                }:
                    offenders.append(f"{path.relative_to(REPO_ROOT)}:{node.lineno}")
            if isinstance(node, ast.Assign) and any(
                is_sys_path(target) for target in node.targets
            ):
                offenders.append(f"{path.relative_to(REPO_ROOT)}:{node.lineno}")
            if isinstance(node, ast.AugAssign) and is_sys_path(node.target):
                offenders.append(f"{path.relative_to(REPO_ROOT)}:{node.lineno}")

    assert not offenders, "Do not mutate sys.path in repo Python code:\n" + "\n".join(
        offenders
    )
