//! kb: kanbrain's command line over the markdown files. Everything it does is `kanbrain-core`'s;
//! this binary only passes it the arguments. The kanban board is `kanbrain`, which takes the same commands.

use kanbrain_core::cli::Program;

fn main() -> anyhow::Result<()> {
    Program { name: "kb" }.main(std::env::args().skip(1))
}
