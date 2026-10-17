//! Tab completion: `completions bash|zsh|fish|powershell` prints a script for the shell, which completes commands
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
        /// PowerShell (5.1 and 7), through `Register-ArgumentCompleter -Native`, each id's title as its tooltip.
        Powershell => "powershell",
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
complete -c {program} -n '_{function}_using add list report' -l tag -x -a '({program} complete tags (commandline -opc)[2..])'
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
        Shell::Powershell => powershell(program, &commands, (&statuses, &units, &groups, &shells)),
    }
}

/// PowerShell's script, which goes as bash's does. Where it offers nothing (`search`'s words, a prefix nothing has),
/// PowerShell offers file names instead, which is left as it is: no match can stand for none.
fn powershell(program: &str, commands: &str, (statuses, units, groups, shells): (&str, &str, &str, &str)) -> String {
    let (by_id, project) = (BY_ID.replace(' ', "', '"), PROJECT);
    format!(
        r#"Register-ArgumentCompleter -Native -CommandName {program}, {program}.exe -ScriptBlock {{
    param($word, $ast, $cursor)
    # `complete` failing (a --project naming nothing) means nothing to offer, not its error over the line.
    $ErrorActionPreference = 'SilentlyContinue'
    # The words before the one completed (all of it, the cursor anywhere in it), without the program: what bash's
    # COMP_WORDS holds between them. A quoted word as the program gets it.
    $words = @($ast.CommandElements | Where-Object {{ $_.Extent.EndOffset -lt $cursor }} | ForEach-Object {{
        if ($_ -is [System.Management.Automation.Language.StringConstantExpressionAst]) {{ $_.Value }} else {{ $_.Extent.Text }}
    }} | Select-Object -Skip 1)
    $prev = if ($words.Count) {{ $words[-1] }} else {{ '' }}
    $cmd = ''
    for ($i = 0; $i -lt $words.Count; $i++) {{
        if ($words[$i] -eq '--project') {{ $i++ }}
        elseif ($words[$i] -notlike '-*') {{ $cmd = $words[$i]; break }}
    }}
    # The program writes UTF-8, which Windows' OEM code pages, PowerShell's reading of a program's output there, garble.
    $encoding = [Console]::OutputEncoding
    [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
    $reply = if ($prev -eq '--project') {{ & {program} complete projects 2>$null }}
    elseif ($prev -eq '--tag') {{ & {program} complete tags @words 2>$null }}
    elseif ($prev -eq '--group') {{ '{groups}' -split ' ' }}
    elseif ($prev -eq '--link') {{ & {program} complete ids @words 2>$null }}
    elseif ($cmd -eq '') {{ '{commands} --project' -split ' ' }}
    elseif ($cmd -eq 'tag') {{
        # The id first, then +TAG and -TAG; options don't count as either.
        $given = 0
        for ($j = $i + 1; $j -lt $words.Count; $j++) {{
            if ($words[$j] -eq '--project') {{ $j++ }}
            elseif ($words[$j] -notlike '--*') {{ $given++ }}
        }}
        if ($given -eq 0) {{ & {program} complete ids @words 2>$null }}
        else {{ & {program} complete tags @words 2>$null | ForEach-Object {{ "+$_"; "-$_" }} }}
    }}
    elseif ($cmd -in '{by_id}') {{ & {program} complete ids @words 2>$null }}
    elseif ($cmd -eq 'add') {{ '--note --decision --tag --link --body --start --json' -split ' ' }}
    elseif ($cmd -eq 'list') {{ '{statuses} all notes decisions --tag --json' -split ' ' }}
    elseif ($cmd -eq 'report') {{ '{units} --group --tag --save' -split ' ' }}
    elseif ($cmd -eq 'project') {{ '{project}' -split ' ' }}
    elseif ($cmd -eq 'init') {{ '--all --repo' -split ' ' }}
    elseif ($cmd -eq 'deinit') {{ '--purge' }}
    elseif ($cmd -eq 'completions') {{ '{shells}' -split ' ' }}
    [Console]::OutputEncoding = $encoding
    # A quote begun comes closed: `'my` as `'my'`.
    $typed = if ($word.StartsWith("'")) {{ $word.Trim("'") -replace "''", "'" }} else {{ $word }}
    foreach ($item in $reply) {{
        # An id comes with its title after a tab: the tooltip.
        $text, $tip = "$item" -split "`t", 2
        if ($text.StartsWith($typed, [System.StringComparison]::Ordinal)) {{
            $tip = if ($tip) {{ $tip }} else {{ $text }}
            # Quoted where PowerShell would read it otherwise: `my app` as two words, `a$b` as `a`.
            $insert = if ($text -match '[\s''"`$;,(){{}}@#&|<>]') {{ "'" + ($text -replace "'", "''") + "'" }} else {{ $text }}
            [System.Management.Automation.CompletionResult]::new($insert, $text, 'ParameterValue', $tip)
        }}
    }}
}}
"#
    )
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
