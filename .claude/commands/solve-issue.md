# Solve an Issue

Pick and solve exactly **one** issue, then create a PR.

## Workflow

1. List open issues, pick **one** (preferring higher-priority / lower number), and **ask the user to confirm your choice before proceeding** (see criteria below):
   ```bash
   for f in issues/*.json; do echo "$(basename "$f" .json): $(jq -r '[.priority, .title] | @tsv' "$f")"; done | sort -t: -k2 -n
   ```
2. Implement the fix
3. Update tests to expect the correct behavior
4. Run `make test` and `make lint` to verify
5. Run `make pipeline` and spot-check the diff for regressions
6. Delete the issue file (e.g., `rm issues/the-issue-name.json`)
7. **Document ALL issues you discover** during exploration, even if you're only fixing one. Future Claudes benefit from this documentation!
8. Create a PR, then stop - leave remaining issues for the next Claude

## Is It Worth Fixing?

Not every quirk deserves a fix. For issues that seem one-in-a-million (e.g. an uncommon typo) or where it's not realistically possible to determine the original author's intent, it's fine to give up and handle it gracefully. Being correct on fewer things is better than being _wrong_.

## Important

- One issue per PR - keeps PRs small and reviewable
- Stop after creating the PR - don't chain multiple fixes
