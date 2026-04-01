# Testing Guide Enrichment

This document describes the procedure for populating the `testing_guide`
field in `requirements.json`.

## Field purpose

The `testing_guide` field contains a detailed description of what needs to
be tested for a given requirement. The requirement's `text` field alone
often lacks sufficient context because it is extracted from a larger section
of the POSIX specification. The testing guide bridges that gap by reading
the surrounding specification text and producing a self-contained
description of the expected behavior.

## Enrichment procedure

When asked to enrich `testing_guide` for a particular requirement, follow
these steps:

### 1. Locate the source file

Open the HTML file referenced by the requirement's `file` field, relative
to `docs/posix/susv5-html/`. For example, if `file` is
`utilities/V3_chap02.html`, open `docs/posix/susv5-html/utilities/V3_chap02.html`.

Markdown versions of the specification are also available at `docs/posix/md/`
(converted from the HTML sources using `html_to_md.rs`) and can be used
for reference as well. The directory structure mirrors `docs/posix/susv5-html/`,
so `utilities/V3_chap02.html` corresponds to `docs/posix/md/utilities/V3_chap02.md`.

### 2. Find the requirement's section

Use the `section_path` array to navigate to the section that contains the
requirement. The last element of `section_path` is the leaf section where
the requirement text appears.

### 3. Read the section with parent context

Read the **full leaf section** (the last element of `section_path`), plus
the **header preamble** of each ancestor section in the path.

A section's header preamble is the text between the section heading and the
start of its first child subsection. This typically contains introductory
definitions, scope statements, or contextual information that affects the
interpretation of all child sections.

For example, given:

```json
"section_path": [
  "2. Shell Command Language",
  "2.5 Parameters and Variables",
  "2.5.3 Shell Variables"
]
```

Read:
- The full text of section **2.5.3 Shell Variables** (from the `2.5.3`
  heading to the next heading at the same or higher level).
- The header of section **2.5 Parameters and Variables** (from the `2.5`
  heading up to the start of `2.5.1`).
- The header of section **2. Shell Command Language** (from the `2.`
  heading up to the start of `2.1`).

If the requirement text or the surrounding section references other
sections or files (e.g. "see 2.6 Word Expansions", "as defined by XBD
8. Environment Variables"), read those referenced sections as well. Follow
cross-references to the extent needed to understand the full behavioral
contract. Referenced sections may be in the same file or in other files
under `docs/posix/susv5-html/`.

### 4. Analyze and write the testing guide

Using the full context gathered above, produce a detailed description of
what needs to be tested. The description should:

- Be self-contained: a reader should understand what to test without
  needing to consult the POSIX specification.
- Cover the specific behavioral obligation stated in the `text` field.
- Include any preconditions, edge cases, or interactions implied by the
  parent section context.
- Note any terminology defined in parent sections that affects
  interpretation of the requirement.
- Describe observable behavior (exit status, stdout, stderr, side effects)
  rather than implementation details.

### 5. Store the result

Set the `testing_guide` field to the resulting description string. The
field is `null` when not yet enriched.

### 6. Write or update tests

Using the `testing_guide` as a specification, write new tests that cover
exactly what the guide describes. If the requirement already has tests
linked in its `tests` array, review them first — only add new tests for
behavior that is not already covered. If the existing tests are
incorrect or insufficient (e.g. testing unrelated behavior), replace
them.

Update the requirement's `tests` array in `requirements.json` to
reference the new tests, and add the corresponding `requirement`
directives and test blocks in the appropriate `.epty` file(s).
