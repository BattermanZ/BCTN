use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "bctn-ingest", version)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Ingest(IngestArgs),
    Search(SearchArgs),
    Show(ShowArgs),
    Top(TopArgs),
    Variants {
        #[command(subcommand)]
        command: VariantsCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum VariantsCommand {
    Build(VariantsBuildArgs),
    BuildEnsemble(VariantsBuildEnsembleArgs),
    Materialize(VariantsMaterializeArgs),
}

#[derive(Debug, Parser)]
pub(crate) struct IngestArgs {
    #[arg(long, default_value = "database_source")]
    pub(crate) database_source_dir: PathBuf,

    #[arg(long, default_value = "data/bctn.sqlite")]
    pub(crate) out: PathBuf,

    #[arg(long, default_value_t = 2005)]
    pub(crate) start_year: i32,

    #[arg(long, default_value_t = 2024)]
    pub(crate) end_year: i32,
}

#[derive(Debug, Parser)]
pub(crate) struct SearchArgs {
    #[arg(long, default_value = "data/bctn.sqlite")]
    pub(crate) db: PathBuf,

    pub(crate) query: String,

    #[arg(long)]
    pub(crate) contains: bool,

    #[arg(long, default_value_t = 25)]
    pub(crate) limit: u32,
}

#[derive(Debug, Parser)]
pub(crate) struct ShowArgs {
    #[arg(long, default_value = "data/bctn.sqlite")]
    pub(crate) db: PathBuf,

    pub(crate) term: String,
}

#[derive(Debug, Parser)]
pub(crate) struct TopArgs {
    #[arg(long, default_value = "data/bctn.sqlite")]
    pub(crate) db: PathBuf,

    #[arg(long)]
    pub(crate) country: String,

    #[arg(long)]
    pub(crate) year: i32,

    #[arg(long)]
    pub(crate) sex: Option<String>,

    #[arg(long, default_value_t = 25)]
    pub(crate) limit: u32,
}

#[derive(Debug, Parser)]
pub(crate) struct VariantsBuildArgs {
    #[arg(long, default_value = "data/bctn.sqlite")]
    pub(crate) db: PathBuf,

    #[arg(long, default_value_t = 1000)]
    pub(crate) max_concepts: u32,

    #[arg(long, default_value_t = 500)]
    pub(crate) request_sleep_ms: u64,

    #[arg(long, default_value_t = 50)]
    pub(crate) batch_size: u32,

    #[arg(long, default_value = "data/wikidata_cache.json")]
    pub(crate) cache_path: PathBuf,

    #[arg(long)]
    pub(crate) refresh_cache: bool,

    #[arg(long)]
    pub(crate) no_clear: bool,

    /// Emit progress to stderr every N search terms / entity batches (0 disables).
    #[arg(long, default_value_t = 50)]
    pub(crate) progress_every: u32,
}

#[derive(Debug, Parser)]
pub(crate) struct VariantsBuildEnsembleArgs {
    #[arg(long, default_value = "data/bctn.sqlite")]
    pub(crate) db: PathBuf,

    #[arg(long, default_value = "ALL")]
    pub(crate) country: String,

    #[arg(long, default_value_t = 50)]
    pub(crate) min_total: i64,

    #[arg(long, default_value_t = 20000)]
    pub(crate) max_concepts: u32,

    #[arg(long, default_value_t = 2)]
    pub(crate) max_edit_distance: usize,

    #[arg(long, default_value_t = 8)]
    pub(crate) min_years_nonzero: usize,

    #[arg(long, default_value_t = 0.98)]
    pub(crate) accept_threshold: f64,

    #[arg(long, default_value_t = 0.85)]
    pub(crate) propose_threshold: f64,

    #[arg(long)]
    pub(crate) clear_ensemble: bool,
}

#[derive(Debug, Parser)]
pub(crate) struct VariantsMaterializeArgs {
    #[arg(long, default_value = "data/bctn.sqlite")]
    pub(crate) db: PathBuf,
}
