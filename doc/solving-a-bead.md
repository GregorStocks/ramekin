# So You've Been Told to Solve a Bead

This document explains how to pick and solve beads.

## One Issue Per PR

Each Claude instance should fix exactly one issue, then create a PR. This keeps PRs small and reviewable.

## The Workflow

1. Pick **one** issue that seems worth fixing (see criteria below)
2. Implement the fix
3. Update tests to expect the correct behavior
4. Run `make test` and `make lint` to verify
5. **Document ALL issues you discover** during exploration, even if you're only fixing one. Future Claudes benefit from this documentation!
6. Create a PR, then stop - leave remaining issues for the next Claude

## Is It Worth Fixing?

Not every quirk deserves a fix. For issues that seem one-in-a-million (e.g. an uncommon typo) or where it's not realistically possible to determine the original author's intent, it's fine to give up and handle it gracefully. Being correct on fewer things is better than being _wrong_.
