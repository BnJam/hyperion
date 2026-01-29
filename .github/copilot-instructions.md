# Copilot instructions for Hyperion

This repo exports skill bundles consumable by agents. Use the generated `<skill>.prompt.md` files at the root of an exported bundle as canonical prompts for automated agents.

How to generate
- cargo build --release
- ./target/release/hyperion export --dest ./bundle --overwrite

Where to find prompts
- Generated prompts: `./bundle/<skill>.prompt.md`
- Skill implementations and additional docs: `./bundle/skills/`

Refer to AGENTS.md for a short guide and examples on using these prompts.
