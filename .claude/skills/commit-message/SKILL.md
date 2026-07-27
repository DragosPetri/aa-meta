---
name: commit-message
description: Generate a commit message from the session context and staged/unstaged git changes
---

Run `git diff HEAD` (or `git diff --cached` if HEAD doesn't exist) to see the outgoing changes, and `git log --oneline -10` to observe the commit style of this repo.

Using the changes and the current conversation context as input, compose a single commit message that:

- Follows the conventional-commits style already used in this repo (e.g. `feat:`, `fix:`, `refactor:`)
- Subject line: imperative mood, ≤72 characters, no trailing period
- Body (if needed): wrap at 72 characters, explain *why* not *what*
- Trailer (always required):

```
Co-authored-by: Claude
```

**Output only the raw commit message text — nothing else.** No preamble, no explanation, no markdown fences. The output must be pasteable directly into `git commit -m "..."` or a commit editor without editing.
