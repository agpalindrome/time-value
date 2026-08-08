---
type: Principle
title: We are this bundle's producer, not its consumer
description:
  A tolerance the spec addresses to a consumer is not a licence for the
  producer, so a finding about our own material is a defect here.
tags: [standing-rule, process]
status: stable
verified: { by: human:ojhermann, at: 2026-08-08T16:01:35Z }
generated: { by: claude/opus-5, at: 2026-08-08T15:58:48Z }
sources:
  - id: okf-spec
    resource: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
    title: Open Knowledge Format — specification, v0.2
    author: team:google-cloud-platform
---

# The rule

Most of what the OKF specification says about rejection is an instruction **to a
consumer**: a broken cross-link, a missing optional family, an out-of-order log
and a concept with no trust frontmatter are all things §6, §9 and §11 say a
consumer MUST NOT reject a bundle over.[^okf-spec] Every one of those
instructions is addressed to somebody reading a bundle they did not write.

We wrote this one.

**Decided — a tolerance addressed to a consumer is not a licence for the
producer.** Where a finding is about material this repo owns — a link we wrote,
a log we keep, a concept we generated — the spec's tolerance does not transfer,
and the finding is a defect here.

**This is the first concept in this bundle to cite the specification.** Every
other source here is finance; the spec had been ambient context. Naming it as a
source is the point of the rule: the tolerances being quoted are the spec's, and
what is done with them is ours.

# The policy this fixes

Codes below are `okf-graph`'s naming. The claim is about the finding, not the
identifier, so a renamed code does not change what this says.

| finding                                                                                                        | here   | why                                                                                                                            |
| -------------------------------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------ |
| a dangling body link, path, index entry or log entry (`BUNDLE-2`, `BUNDLE-3`, `INDEX-2`, `LOG-3`)              | defect | Our links, in our bundle. §6's tolerance exists for a reader who cannot write the missing document; we can.                    |
| a log whose dates are not newest-first (`LOG-2`)                                                               | defect | §9 states the order and declines to make it a MUST. The log is ours to keep ordered, so the omission is not our exemption.     |
| an `okf_version` the checker does not understand (`INDEX-3`)                                                   | report | A statement about the checker's vintage rather than about the bundle. Failing on it would gate our work on a tool's age.       |
| a derivation cycle (`BUNDLE-4`)                                                                                | report | Whether a cycle is benign or a contradiction is an open question upstream. Denying it would answer that question silently.     |
| an incomplete attestation contract or an unusable credibility signal (`CONCEPT-9`, `CONCEPT-10`, `CONCEPT-14`) | report | Nothing here has an Attested Computation or a `usage_count`. A policy about a surface we do not use is a policy about nothing. |

**Derived — the distinction is ownership, not severity.** The first two rows are
about material this repo authored; the last three are about a tool's vintage, an
unsettled question, and a surface we do not use. None of the three is ours to
fix, which is why none of them is a defect here.

# It already had four instances before it was written down

The rule is not new; only its statement is. Every house rule this repo holds its
bundle to is an instance of it, and each was justified locally, in isolation:

- **`generated.at` is required**, where §4.1 makes the whole family optional and
  §5.2 marks only `by` required.
- **A `stable` concept must carry a verification**, where §5.3 says a concept
  with no trust frontmatter is still consumable and MUST NOT be rejected.
- **A verification must not predate `generated.at`**, where §5.2 states the two
  are independent and calls the gap ordinary.
- **A tolerated finding must not pass in silence**, adopted when the checker
  moved into a test suite and its printed output stopped being read.

Four arguments, each of the form "the spec says a consumer may accept this, and
we do not". Writing the shared rule down is what makes the fifth cheap and the
next one consistent — see
[code and bundle change together](code-and-bundle-change-together.md), whose
third question is whether a fix is an instance of a rule nobody wrote.

# Limits

**Our strictness must never make this bundle non-conformant.** The rule runs one
way: it adds requirements to what we accept, never permissions to what we emit.
A bundle that fails our checks may be perfectly conformant, and one that passes
them must be. If a house rule ever required something the spec forbids, the
house rule is wrong.

**It says nothing about anybody else's bundle.** Should this repo ever read a
bundle it did not write, §6 and §11 apply in full and a dangling link is
tolerated. The strictness is a property of the relationship, not of the checker.

**It does not make the checker's verdict wrong.** `okf-graph` reporting a
tolerated finding rather than failing is the spec implemented correctly. The
policy is a layer above it — a consumer's decision, as the checker's own levels
frame it — and not a re-reading of §11.

**It is not a licence for a rule nobody can act on.** A finding is a defect here
because we can fix it. That test is what keeps the last three rows of the table
above from drifting upward whenever a run looks noisy.

[^okf-spec]:
    _Open Knowledge Format specification_, v0.2. §6 on links and paths, §9 on
    the log, §11 on conformance, and §5.3 on a concept with no trust
    frontmatter. The tolerances quoted are all addressed to a consumer; the
    specification says nothing about what a producer should demand of itself.
