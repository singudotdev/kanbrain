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

/// The commands whose first argument is an entry's id; `link` takes more after it.
const BY_ID: &str = "show start park done todo comment ask wait propose accept reject describe link";

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
            r"complete -c {program} -f
complete -c {program} -n __fish_use_subcommand -a '{commands}'
complete -c {program} -l project -x -a '({program} complete projects)'
complete -c {program} -l tag -x -a '({program} complete tags (commandline -opc)[2..])'
complete -c {program} -l json
complete -c {program} -n '__fish_seen_subcommand_from {BY_ID}' -a '({program} complete ids (commandline -opc)[2..])'
complete -c {program} -n '__fish_seen_subcommand_from add' -l link -x -a '({program} complete ids (commandline -opc)[2..])'
complete -c {program} -n '__fish_seen_subcommand_from add' -l note -l decision -l start -l body
complete -c {program} -n '__fish_seen_subcommand_from list' -a '{statuses} all notes decisions'
complete -c {program} -n '__fish_seen_subcommand_from report' -a '{units}' -l save
complete -c {program} -n '__fish_seen_subcommand_from report' -l group -x -a '{groups}'
complete -c {program} -n '__fish_seen_subcommand_from project' -a '{project}'
complete -c {program} -n '__fish_seen_subcommand_from project' -l purge
complete -c {program} -n '__fish_seen_subcommand_from init' -l all -d 'in every registered project'
complete -c {program} -n '__fish_seen_subcommand_from init' -l repo -x
complete -c {program} -n '__fish_seen_subcommand_from deinit' -l purge
complete -c {program} -n '__fish_seen_subcommand_from wait' -l timeout -x
complete -c {program} -n '__fish_seen_subcommand_from completions' -a '{shells}'
"
        ),
    }
}

#[cfg(test)]
#[path = "../../tests/cli/completions.rs"]
mod tests;
