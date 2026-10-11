#!/usr/bin/env bash
# End-to-end check that a real coding agent works with kanbrain the way `init` tells it to: Claude Code, run
# headless on small throwaway projects, one scenario each, then judged by what landed in `.brain/` and the code.
#
#   e2e/agent.sh                 every scenario
#   e2e/agent.sh plan decision   only those
#
# It calls the model, so it costs money and takes minutes, and an agent's choices vary between runs: it's run by
# hand before a release that changes what agents are told (the section `init` writes, `context`, the hooks), not
# by CI. Each scenario's transcript is kept in the work folder it prints.
#
# Environment: E2E_MODEL (default: Claude Code's), E2E_BUDGET (dollars per scenario, default 1),
# KANBRAIN_WAIT_TIMEOUT (minutes an agent's `kb wait` lasts, default 1 here).
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cargo build -q --manifest-path "$repo/Cargo.toml" --bin kb
# The agent and the hooks run `kb`: this build's, not an installed one.
export PATH="$repo/target/debug:$PATH"

work=$(mktemp -d "${TMPDIR:-/tmp}/kb-e2e.XXXXXX")
export KANBRAIN_DIR="$work/central"
# Nobody answers here but the `wait` scenario, within seconds: an agent that waits for the user otherwise holds the
# scenario for `kb wait`'s 8 minutes.
export KANBRAIN_WAIT_TIMEOUT="${KANBRAIN_WAIT_TIMEOUT:-1}"
echo "work folder: $work"

passed=0
failed=0

# A small Python app with a README in a new folder `$1`, made a repository of project `$2` by `kb init`. Each
# scenario has its own project, so none sees another's repositories or card.
app() {
    mkdir -p "$1"
    cat >"$1/README.md" <<'EOF'
# notes-api

A tiny HTTP API that stores short text notes in memory. `python3 app.py` serves it on port 8000.
EOF
    cat >"$1/app.py" <<'EOF'
"""A tiny notes API: GET /notes lists them, POST /notes adds one."""
import json
from http.server import BaseHTTPRequestHandler, HTTPServer

NOTES = []


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/notes":
            self.reply(200, NOTES)
        else:
            self.reply(404, {"error": "not found"})

    def do_POST(self):
        if self.path == "/notes":
            length = int(self.headers.get("Content-Length", 0))
            NOTES.append(self.rfile.read(length).decode())
            self.reply(201, {"count": len(NOTES)})
        else:
            self.reply(404, {"error": "not found"})

    def reply(self, status, body):
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(body).encode())


if __name__ == "__main__":
    HTTPServer(("", 8000), Handler).serve_forever()
EOF
    (cd "$1" && kb init "$2" >/dev/null)
}

# Describes the project of repository `$1`, and the repository's role (`$2`, else the API's), as an earlier
# session would have.
described() {
    (cd "$1" && kb project describe "A notes service: a Python HTTP API and a web front end." >/dev/null)
    (cd "$1" && kb project notes "${2:-The HTTP API.}" >/dev/null)
}

# Runs the agent in folder `$1` with prompt `$2`; its answer goes to `$1.log`.
agent() {
    local args=(-p "$2" --no-session-persistence --permission-mode acceptEdits
        --allowedTools "Bash(kb:*)" "Bash(python3:*)" Read Edit Write Glob Grep
        --max-budget-usd "${E2E_BUDGET:-1}")
    if [ -n "${E2E_MODEL:-}" ]; then args+=(--model "$E2E_MODEL"); fi
    (cd "$1" && claude "${args[@]}" >"$1.log" 2>&1) || true
}

# Judges a scenario: the Python in `$2` reads the brain of folder `$1` (as `brain`, every entry in full) and
# raises AssertionError with what's wrong.
check() {
    local name=$1 dir=$2 code=$3
    if (cd "$dir" && python3 - "$code" <<'EOF'); then
import json, subprocess, sys

def kb(*args):
    return subprocess.run(["kb", *args], capture_output=True, text=True, check=True).stdout

summaries = json.loads(kb("list", "all", "notes", "decisions", "--json"))
brain = [json.loads(kb("show", str(s["id"]), "--json")) for s in summaries]
tasks = [e for e in brain if e["kind"] == "task"]
by_status = lambda status: [t for t in tasks if t.get("status") == status]
project = kb("project")
exec(sys.argv[1])
EOF
        echo "PASS $name"
        passed=$((passed + 1))
    else
        echo "FAIL $name (transcript: $dir.log)"
        failed=$((failed + 1))
    fi
}

# Whether scenario `$1` was asked for.
wanted() {
    [ ${#selected[@]} -eq 0 ] || [[ " ${selected[*]} " == *" $1 "* ]]
}
selected=("$@")

# A first session in an undescribed project, asked for a change: the hooks hold it to describing the project
# first; then the change gets a task, started, commented on and finished.
if wanted implement; then
    dir="$work/implement"
    app "$dir" implement
    agent "$dir" "Add a GET /health endpoint that returns {\"status\": \"ok\"}."
    check implement "$dir" '
assert "(empty)" not in project.split("## Repository")[0], "the project was described: " + project
description = project.split("\n\n", 1)[1].split("## Repository")[0].strip()
assert len(description.splitlines()) <= 6, "a short description: " + description
done = by_status("done")
assert done, "the change has a task, done"
assert any(t.get("comments") for t in done), "commented on as it went"
assert not by_status("doing"), "nothing left running"
assert "health" in open("app.py").read(), "and the endpoint is there"
'
fi

# Asked for a plan: a task per item, in To do, and no code.
if wanted plan; then
    dir="$work/plan"
    app "$dir" plan
    described "$dir"
    before=$(cat "$dir/app.py")
    agent "$dir" "Plan how to add user accounts with login to this API. List the todos; don't implement anything yet."
    check plan "$dir" '
assert len(by_status("todo")) >= 2, "each item a task in To do"
assert not by_status("doing") and not by_status("done"), "none started"
'
    [ "$(cat "$dir/app.py")" = "$before" ] || echo "  note: plan changed app.py"
fi

# A question: an answer, and nothing on the board.
if wanted question; then
    dir="$work/question"
    app "$dir" question
    described "$dir"
    agent "$dir" "What does POST /notes return?"
    check question "$dir" '
assert not brain, "a question gets no entries: " + str([e["title"] for e in brain])
'
fi

# Choosing between approaches: a decision with its reasons, in the ADR sections.
if wanted decision; then
    dir="$work/decision"
    app "$dir" decision
    described "$dir"
    agent "$dir" "Notes are lost on restart. Decide between SQLite and a JSON file to keep them, and record why. Don't implement it yet."
    check decision "$dir" '
decisions = [e for e in brain if e["kind"] == "decision"]
assert decisions, "a decision"
text = decisions[0].get("description", "")
assert all(s in text for s in ("Context", "Decision", "Consequences")), "with its sections: " + text
'
fi

# A requirement that's the user's to set: the agent asks on the board instead of guessing, and stops there.
if wanted ask; then
    dir="$work/ask"
    app "$dir" ask
    described "$dir"
    agent "$dir" "Notes need a maximum length. Which maximum is my call, and I'm away until tomorrow."
    check ask "$dir" '
waiting = [t for t in tasks if t.get("asked") == "waiting for an answer"]
assert waiting, "a task waits for the answer: " + str([(t["title"], t.get("status")) for t in tasks])
assert any(c.get("question") and c.get("author") == "agent" for c in waiting[0]["comments"]), "with the question"
'
fi

# A storage choice nobody made: proposed for the user, not taken alone.
if wanted propose; then
    dir="$work/propose"
    app "$dir" propose
    described "$dir"
    agent "$dir" "Notes are lost on restart. Make them persistent."
    check propose "$dir" '
proposed = [e for e in brain if e.get("decision") == "proposed"]
assert proposed, "a proposed decision: " + str([(e["title"], e.get("decision")) for e in brain if e["kind"] == "decision"])
assert any(t.get("asked") == "waiting for an answer" for t in tasks), "its task waits on it"
'
fi

# The user is at the board: the agent asks, waits with `kb wait` in the background, and goes on with the answer
# in the same session. The answer comes from here, as a person, a few seconds after the question shows up.
if wanted wait; then
    dir="$work/wait"
    app "$dir" wait
    described "$dir"
    (cd "$dir" && KANBRAIN_AUTHOR=human python3 - <<'EOF') &
import json, subprocess, time

def kb(*args):
    return subprocess.run(["kb", *args], capture_output=True, text=True, check=True).stdout

for _ in range(600):
    for summary in json.loads(kb("list", "parked", "--json")):
        if json.loads(kb("show", str(summary["id"]), "--json")).get("asked") == "waiting for an answer":
            time.sleep(5)
            kb("comment", str(summary["id"]), "280 characters; reject longer ones with 400.")
            raise SystemExit
    time.sleep(1)
EOF
    answerer=$!
    agent "$dir" "Notes need a maximum length. Which maximum is my call: ask me, I'm watching the board."
    # Still running only if no question ever showed up.
    kill "$answerer" 2>/dev/null || true
    wait "$answerer" || true
    check wait "$dir" '
asked = [t for t in tasks if any(c.get("question") for c in t["comments"])]
assert asked, "it asked: " + str([(t["title"], t.get("status")) for t in tasks])
assert asked[0]["status"] in ("doing", "done"), "and went on in the same session: " + asked[0]["status"]
assert "280" in open("app.py").read(), "with the answer"
'
fi

# The user answered a question while the agent was away: the next session picks the task up with the answer.
if wanted answered; then
    dir="$work/answered"
    app "$dir" answered
    described "$dir"
    # Who wrote what is set by hand: this script may itself run inside an agent.
    (cd "$dir" && KANBRAIN_AUTHOR=agent kb add --start "Limit note length" >/dev/null &&
        KANBRAIN_AUTHOR=agent kb ask 1 "Which maximum length for a note?" >/dev/null &&
        KANBRAIN_AUTHOR=human kb comment 1 "280 characters; reject longer ones with 400." >/dev/null)
    agent "$dir" "Go on with the project."
    check answered "$dir" '
task = next(t for t in tasks if t["id"] == 1)
assert task["status"] in ("doing", "done"), "the answered task was picked up: " + task["status"]
assert "280" in open("app.py").read(), "with the answer"
'
fi

# A project of two repositories: work the other one needs becomes a task there, linked to this one's.
if wanted repos; then
    api="$work/repos/api"
    web="$work/repos/web"
    # One project, its own: every scenario has its own, so none sees another's repositories.
    app "$api" shop
    app "$web" shop
    described "$api"
    described "$web" "The web front end: pages that call the API."
    agent "$web" "The page header must show how many notes there are, but the API has no endpoint for the count. \
Work on the header here, and leave the API change to the API repository: don't edit its code."
    check repos "$api" '
assert tasks, "the API work is a task in the API repository"
'
    check repos-link "$web" '
assert tasks, "the header has a task here"
linked = any(link.startswith("api#") for t in tasks for link in t.get("repo_links", []))
api = json.loads(kb("--project", "shop/api", "list", "all", "--json"))
back = any(link.startswith("web#") for s in api for link in json.loads(kb("--project", "shop/api", "show", str(s["id"]), "--json")).get("repo_links", []))
assert linked or back, "the two tasks are linked"
'
fi

# A new session after one that stopped halfway: it picks up the task that was running, from its last comment.
if wanted resume; then
    dir="$work/resume"
    app "$dir" resume
    described "$dir"
    (cd "$dir" && kb add --start "Add a DELETE /notes endpoint that clears every note" >/dev/null &&
        kb comment 1 "Handler not written yet; next: add do_DELETE to Handler." >/dev/null)
    agent "$dir" "Continue where we left off."
    check resume "$dir" '
task = next(t for t in tasks if t["id"] == 1)
assert len(task.get("comments", [])) >= 2 or task["status"] == "done", "the running task was picked up"
assert "do_DELETE" in open("app.py").read(), "and its work done"
'
fi

echo "$passed passed, $failed failed"
[ "$failed" -eq 0 ]
