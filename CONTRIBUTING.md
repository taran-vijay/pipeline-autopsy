# Contributing

Start with [local setup](docs/development.md) and [architecture](docs/architecture.md).
The current scope and implemented API routes are recorded in the README and
[API guide](docs/api.md).

1. Make a focused change on a short-lived branch.
2. Add meaningful tests for changed behavior and update affected documentation.
3. Run the Rust and frontend checks in the development guide.
4. Review `git status`, the diff, and any new files. Include lockfile/schema changes
   with the source changes that require them.
5. Commit, push the branch, and open a pull request. Review both CI job results.

Keep runtime databases, evidence exports, secrets, and generated build output out
of commits. Use small synthetic fixtures that contain no sensitive records.
Preserve crate dependency direction and add a new migration instead of editing an
applied one. Regenerate OpenAPI after changing the public wire contract.

For a ready-to-review change, the terminal workflow is:

```sh
git add .
git diff --cached
git commit -m "Describe the change"
git push -u origin HEAD
```

Inspect staged files before committing. Repository branch protection settings are
managed separately from the CI workflow.
