mod ensemble;
mod ensemble_score;
mod wikidata;

use anyhow::Result;

use crate::cli::{VariantsBuildArgs, VariantsBuildEnsembleArgs};

pub(crate) fn run_build(args: VariantsBuildArgs) -> Result<()> {
    wikidata::run_build(args)
}

pub(crate) fn run_build_ensemble(args: VariantsBuildEnsembleArgs) -> Result<()> {
    ensemble::run_build_ensemble(args)
}
