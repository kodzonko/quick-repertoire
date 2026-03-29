# Agent instructions

- After introducing changes run `uv run ruff format`, `uv run ruff check --fix` and `uv run ty check`. Make sure all 3 pass without errors.
- After completing the implementation make sure it is covered by tests and all tests pass. If you broke any existing tests, fix them and make sure they pass as well.
- Prefer integration and e2e tests over heavily patched unit tests. Use unit tests when certain conditions are hard to reproduce without patching or to test specific edge cases. In general, try to avoid patching as much as possible.
- Do not report task as done until it is tested and all tests pass.
- If you spot a bug or regression first implement a test reproducing the error (which should fail) and then fix the bug and make sure the test passes.
