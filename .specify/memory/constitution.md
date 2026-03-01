<!--
## Sync Impact Report

- **Version change**: 0.0.0 → 1.0.0 (initial ratification)
- **Modified principles**: N/A (initial creation)
- **Added sections**: 
  - I. Code Quality
  - II. Test Standards  
  - III. User Experience Consistency
  - IV. Performance Requirements
  - Development Workflow
  - Quality Gates
  - Governance
- **Removed sections**: None
- **Templates status**:
  - ✅ plan-template.md: Constitution Check section compatible
  - ✅ spec-template.md: Requirements and success criteria align with principles
  - ✅ tasks-template.md: Task phases support principle-driven workflow
- **Follow-up TODOs**: None
-->

# Biblio Constitution

## Core Principles

### I. Code Quality

- Code MUST pass linting and type checking before merge (zero warnings)
- Code MUST follow consistent formatting enforced by automated tools
- Functions MUST have single responsibility and be under 50 lines where practical
- All public APIs MUST be documented with purpose, parameters, return values, and examples
- Code MUST be readable and self-documenting; comments explain "why" not "what"
- Technical debt MUST be tracked explicitly and addressed within 2 sprints

**Rationale**: Maintainable code reduces long-term cost and enables team scalability.

### II. Test Standards (NON-NEGOTIABLE)

- Test-Driven Development MUST be followed: write failing tests → approve → implement → pass
- Unit test coverage MUST be ≥80% for all new code; critical paths MUST have 100% coverage
- Integration tests MUST cover all external contracts and API boundaries
- All tests MUST be deterministic: no flaky tests allowed in main branch
- Tests MUST run in isolation without external dependencies (use mocks/stubs)
- Test names MUST describe the scenario and expected outcome clearly

**Rationale**: Tests are executable documentation and safety net for refactoring.

### III. User Experience Consistency

- All user-facing interfaces MUST follow a unified design system
- Error messages MUST be actionable, specific, and user-friendly (no stack traces to users)
- Response times MUST be consistent; operations >1s MUST show progress indication
- Accessibility MUST meet WCAG 2.1 Level AA standards minimum
- User workflows MUST be validated against documented user journeys
- Breaking UX changes MUST be flagged in PR reviews with migration guidance

**Rationale**: Consistent UX builds trust and reduces user friction.

### IV. Performance Requirements

- API endpoints MUST respond within 200ms (p95) for read operations
- API endpoints MUST respond within 500ms (p95) for write operations
- Memory usage MUST not exceed 512MB per service instance under normal load
- Database queries MUST use indexed fields; full table scans require explicit justification
- Large operations MUST support pagination; default page size ≤50 items
- Performance regressions MUST be caught by automated benchmarking in CI

**Rationale**: Performance is a feature; regressions erode user trust.

## Development Workflow

- Branch naming: `###-feature-name` pattern aligned with spec folders
- Commits MUST reference task IDs or issue numbers
- Pull requests MUST include: summary, test evidence, breaking changes noted
- Code reviews MUST be completed within 24 hours
- Deployments MUST follow staged rollout: dev → staging → production
- Rollback procedures MUST be documented and tested for each deployment

## Quality Gates

- **Pre-commit**: Linting, formatting, secret scanning
- **Pre-merge**: Unit tests pass, coverage threshold met, type check passes
- **Pre-deploy**: Integration tests pass, performance benchmarks within threshold
- **Post-deploy**: Smoke tests, error rate monitoring, latency verification

## Governance

This constitution supersedes all other development practices and conventions.

**Amendment Process**:
1. Proposed changes MUST be documented with rationale and impact analysis
2. Amendments MUST be reviewed by at least 2 team members
3. Breaking changes MUST include migration plan and timeline
4. All amendments MUST update dependent templates within same PR

**Version Policy**:
- MAJOR: Backward incompatible principle changes or removals
- MINOR: New principles added or existing principles materially expanded
- PATCH: Clarifications, wording improvements, typo fixes

**Compliance**: All PRs and code reviews MUST verify adherence to these principles. Complexity that violates principles MUST be justified in the Complexity Tracking section of the implementation plan.

**Version**: 1.0.0 | **Ratified**: 2026-02-27 | **Last Amended**: 2026-02-27