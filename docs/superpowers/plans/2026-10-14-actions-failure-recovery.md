# GitHub Actions Failure Recovery Plan

> Inline execution was explicitly requested; this plan is being implemented in the current checkout.

**Goal:** Make checks in the fork useful without unavailable upstream credentials and fix the confirmed formatting and compose workflow failures.

**Architecture:** Keep deterministic checks running; skip credentialed test/publish automations when their required secrets or upstream repository context are absent. Correct the docker-compose env generation and format the files reported by pre-commit.

**Tech Stack:** GitHub Actions YAML, pre-commit, Ruff.

**Spec:** GitHub Actions failures reviewed in this conversation.

## Global Constraints
- Preserve current untracked workspace files.
- Do not add or run tests unless requested.
- Do not expose secret values in workflow logs.

## Review Focus
- Missing LLM secrets must not turn all secret-dependent CI jobs into failures.
- Deterministic CI jobs must still run and gate the summary.
- Publishing and external integrations must run only where their credentials and target repositories are available.
- Empty optional `LLM_ARGS` and no LLM credentials must not break the Compose smoke test.
- Pre-commit changes must be limited to the files it reported.

### Task 1: Normalize Compose and formatter failures
- Update `.github/workflows/docker_compose.yml` to tolerate absent optional secrets and skip only the live LLM answer step when credentials are absent.
- Apply the exact whitespace, EOF, and Ruff fixes reported by the current pre-commit run.

### Task 2: Make Test Suites fork-safe
- Add a credential-presence output and gate suites that depend on live provider credentials.
- Keep deterministic checks active and make the summary require only checks that actually ran.

### Task 3: Guard upstream-only workflows
- Prevent Docker Hub publishing, release notifications/docs writes, and upstream telemetry/sync automations from running in this fork.

### Task 4: Review and validate
- Inspect the diff, validate changed workflow YAML, and run only targeted pre-commit hooks; no test suites unless requested.
