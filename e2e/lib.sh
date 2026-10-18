# shellcheck shell=bash
# What the e2e scripts share, sourced by them (it runs nothing itself).

# The variables that make `kb` take whoever runs it for an agent: `AGENT_VARIABLES` in core/src/cli/mod.rs, kept in
# the same order, and `KANBRAIN_AUTHOR`, which says so outright. A script running `kb` as a person clears them, so
# none leaks in from the shell it runs in (an agent's).
agent_variables=(AGENT AI_AGENT CLAUDECODE CODEX_SANDBOX CODEX_CI CODEX_THREAD_ID GEMINI_CLI KANBRAIN_AUTHOR)

# `env`'s options that clear them: `env "${without_agent[@]}" kb …`.
# shellcheck disable=SC2034 # used by the scripts that source this
without_agent=()
for var in "${agent_variables[@]}"; do without_agent+=(-u "$var"); done
unset var

# Stops the script, saying why.
fail() {
    echo "FAIL: $*" >&2
    exit 1
}
