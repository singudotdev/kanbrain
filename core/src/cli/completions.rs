//! Tab completion: `completions bash|zsh|fish` prints a script for the shell, which completes commands
//! and options from the lists here, and ids, projects and tags by asking `complete` for them as it goes.

use crate::brain::{Brain, Group};
use crate::enums::named_enum;
use crate::model::Status;
use crate::period::Unit;
use crate::projects;
use crate::store::Store;
use anyhow::Result;
use std::path::Path;

/// Every command, as the scripts offer them first.
const COMMANDS: &[&str] = &[
    "add",
    "show",
    "search",
    "start",
    "park",
    "done",
    "todo",
    "comment",
    "ask",
    "wait",
    "propose",
    "accept",
    "reject",
    "describe",
    "rename",
    "tag",
    "link",
    "project",
    "context",
    "list",
    "report",
    "status",
    "init",
    "deinit",
    "path",
    "update",
    "completions",
];

/// The commands whose first argument is an entry's id; `link` takes more after it. `tag` is apart: the id, then signed
/// tags.
const BY_ID: &str = "show start park done todo comment ask wait propose accept reject describe rename link";

named_enum! {
    /// A shell `completions` writes a script for.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Shell {
        /// Bash, through `complete -F`.
        Bash => "bash",
        /// Zsh, through its bash compatibility.
        Zsh => "zsh",
        /// Fish, with each id's title shown beside it.
        Fish => "fish",
    }
}

named_enum! {
    /// What `complete` lists, one per line.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Candidates {
        /// Every entry of the brain, open tasks first, each part newest first, as `ID<tab>title`.
        Ids => "ids",
        /// Every name `--project` takes: `central`, each project of one repository, and each repository of a project
        /// with several (`shop/api`), not the project's name alone, which it refuses.
        Projects => "projects",
        /// Every tag in use in the brain.
        Tags => "tags",
    }
}

/// The candidates, from the brain `store` holds, read without repairing it: a tab press shouldn't write;
/// `Projects` reads the central brain's list instead.
pub fn candidates(what: Candidates, store: Store, central: &Path) -> Result<Vec<String>> {
    Ok(match what {
        Candidates::Ids => {
            let brain = Brain::read_only(store)?;
            let mut entries: Vec<_> = brain.tasks.iter().collect();
            // Open tasks first: they're what status changes and comments are for. Then the rest, notes and decisions
            // included.
            entries.sort_by_key(|t| (!(t.is_task() && t.status != Status::Done), std::cmp::Reverse(t.id)));
            entries.iter().map(|t| format!("{}\t{}", t.id, t.title)).collect()
        }
        Candidates::Projects => {
            let repos = projects::load(central)?;
            let mut names = vec![projects::CENTRAL.to_string()];
            names.extend(repos.iter().map(|r| r.label(&repos)));
            names
        }
        Candidates::Tags => Brain::read_only(store)?.all_tags(),
    })
}

/// What `project` takes after it.
const PROJECT: &str = "describe notes rename repo move remove";

/// The script for `shell`, completing `program`.
pub fn script(shell: Shell, program: &str) -> String {
    let commands = COMMANDS.join(" ");
    let (statuses, units, groups, shells) =
        (Status::names(" "), Unit::names(" "), Group::names(" "), Shell::names(" "));
    let function = format!("_{}", program.replace('-', "_"));
    let project = PROJECT;
    let bash = format!(
        r#"{function}() {{
    local cur=${{COMP_WORDS[COMP_CWORD]}} prev=${{COMP_WORDS[COMP_CWORD-1]}}
    local words=("${{COMP_WORDS[@]:1:COMP_CWORD-1}}") cmd="" i
    for ((i = 1; i < COMP_CWORD; i++)); do
        case ${{COMP_WORDS[i]}} in
            --project) ((i++)) ;;
            -*) ;;
            *) cmd=${{COMP_WORDS[i]}}; break ;;
        esac
    done
    local reply
    case $prev in
        --project) reply=$({program} complete projects) ;;
        --tag) reply=$({program} complete tags "${{words[@]}}") ;;
        --group) reply="{groups}" ;;
        --link) reply=$({program} complete ids "${{words[@]}}" | cut -f1) ;;
        *)
            case $cmd in
                "") reply="{commands} --project" ;;
                tag)
                    # The id first, then +TAG and -TAG; options don't count as either.
                    local given=0 j
                    for ((j = i + 1; j < COMP_CWORD; j++)); do
                        case ${{COMP_WORDS[j]}} in
                            --project) ((j++)) ;;
                            --*) ;;
                            *) ((given++)) ;;
                        esac
                    done
                    if ((given == 0)); then
                        reply=$({program} complete ids "${{words[@]}}" | cut -f1)
                    else
                        reply=$({program} complete tags "${{words[@]}}" | sed 's/^/+/; p; s/^+/-/')
                    fi
                    ;;
                {by_id}) reply=$({program} complete ids "${{words[@]}}" | cut -f1) ;;
                add) reply="--note --decision --tag --link --body --start --json" ;;
                list) reply="{statuses} all notes decisions --tag --json" ;;
                report) reply="{units} --group --tag --save" ;;
                project) reply="{project}" ;;
                init) reply="--all --repo" ;;
                deinit) reply="--purge" ;;
                completions) reply="{shells}" ;;
            esac
            ;;
    esac
    COMPREPLY=($(compgen -W "$reply" -- "$cur"))
}}
complete -F {function} {program}
"#,
        by_id = BY_ID.replace(' ', "|"),
    );
    match shell {
        Shell::Bash => bash,
        Shell::Zsh => format!("autoload -U +X bashcompinit && bashcompinit\n{bash}"),
        Shell::Fish => format!(
            r#"{words}complete -c {program} -f
complete -c {program} -n 'test (count (_{function}_words)) -eq 0' -a '{commands}'
complete -c {program} -l project -x -a '({program} complete projects)'
complete -c {program} -l tag -x -a '({program} complete tags (commandline -opc)[2..])'
complete -c {program} -l json
complete -c {program} -n '_{function}_using {BY_ID}' -a '({program} complete ids (commandline -opc)[2..])'
complete -c {program} -n '_{function}_using tag; and test (count (_{function}_words)) -eq 1' -a '({program} complete ids (commandline -opc)[2..])'
complete -c {program} -n '_{function}_using tag; and test (count (_{function}_words)) -gt 1' -a '({program} complete tags (commandline -opc)[2..] | sed "s/^/+/; p; s/^+/-/")'
complete -c {program} -n '_{function}_using add' -l link -x -a '({program} complete ids (commandline -opc)[2..])'
complete -c {program} -n '_{function}_using add' -l note -l decision -l start -l body
complete -c {program} -n '_{function}_using list' -a '{statuses} all notes decisions'
complete -c {program} -n '_{function}_using report' -a '{units}'
complete -c {program} -n '_{function}_using report' -l save
complete -c {program} -n '_{function}_using report' -l group -x -a '{groups}'
complete -c {program} -n '_{function}_using project; and test (count (_{function}_words)) -eq 1' -a '{project}'
complete -c {program} -n '_{function}_using project' -l purge
complete -c {program} -n '_{function}_using init' -l all -d 'in every registered project'
complete -c {program} -n '_{function}_using init' -l repo -x
complete -c {program} -n '_{function}_using deinit' -l purge
complete -c {program} -n '_{function}_using wait' -l timeout -x
complete -c {program} -n '_{function}_using completions' -a '{shells}'
"#,
            words = fish_words(&function),
        ),
    }
}

/// The functions fish's script asks which command the line is: `__fish_seen_subcommand_from` matches a word anywhere
/// on it, so `project rename` or a title's `show` would be taken for the command.
fn fish_words(function: &str) -> String {
    format!(
        r"function _{function}_words -d 'the command, then the words after it that are not --options'
    set -l words (commandline -opc)[2..]
    set -l found
    while set -q words[1]
        switch $words[1]
            case --project
                set words $words[3..]
            case '--*'
                set words $words[2..]
            case '-*'
                set -q found[1]; and set -a found $words[1]
                set words $words[2..]
            case '*'
                set -a found $words[1]
                set words $words[2..]
        end
    end
    string join \n -- $found
end
function _{function}_using -d 'whether the command is one of the arguments'
    set -l words (_{function}_words)
    set -q words[1]; and contains -- $words[1] $argv
end
"
    )
}

#[cfg(test)]
#[path = "../../tests/cli/completions.rs"]
mod tests;
