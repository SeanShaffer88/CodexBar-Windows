# Compatibility

- **Windows 10 and 11, x64.** The binary statically links the C runtime,
  so no redistributable or other runtime install is needed.
- **Codex CLI**: reads `~/.codex/sessions/**/*.jsonl` `token_count`
  events. If Codex changes that format, the provider reports an honest
  "no usage yet" instead of wrong numbers.
- **Claude Code**: reads `~/.claude/projects/**/*.jsonl` assistant
  message usage blocks, deduplicated by message id.
- **OpenRouter**: `GET https://openrouter.ai/api/v1/auth/key`.
- Custom data locations are supported through `claude_dir` / `codex_dir`
  in `data/config.json` (for example, WSL or multi-user setups).
