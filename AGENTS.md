# Agents and skill prompts

`hyperion export` now generates per-skill prompt files named `<skill>.prompt.md` at the bundle root of the exported bundle. These files are intended to be consumed by agents and automation to learn how to run and interact with the Hyperion system for a particular skill.

Location
- Generated prompt files: `<bundle-root>/<skill>.prompt.md`
- Skill implementations: `<bundle-root>/skills/<skill>/`

Usage
- Run `hyperion export --dest ./bundle --overwrite` to produce the bundle and prompts.
- Inspect the generated `<skill>.prompt.md` to discover CLI commands and usage examples for automating the skill.

For more in-depth developer-facing documentation, inspect the `skills/` directory inside the exported bundle which contains implementation details and any skill-specific README files.
