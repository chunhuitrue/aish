# AISH.md Discovery

Aish looks for guidance in your Aish home directory (usually `~/.aish`; set `AISH_HOME` to change it). This page explains how AISH.md files are discovered and used.

## Global Instructions (`~/.aish`)

- Aish looks for global guidance in your Aish home directory (usually `~/.aish`; set `AISH_HOME` to change it).
- If an `AISH.override.md` file exists there, it takes priority. If not, Aish falls back to `AISH.md`.
- Only the first non-empty file is used.
- Whatever Aish finds here stays active for the whole session.

For a quick overview, see the [Memory with AISH.md section](../docs/getting-started.md#memory-with-agentsmd) in the getting started guide.
