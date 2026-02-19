mod ensemble;
mod ensemble_score;
mod materialize;
mod wikidata;

use anyhow::Result;

use crate::cli::{VariantsBuildArgs, VariantsBuildEnsembleArgs, VariantsMaterializeArgs};

pub(crate) fn run_build(args: VariantsBuildArgs) -> Result<()> {
    wikidata::run_build(args)
}

pub(crate) fn run_build_ensemble(args: VariantsBuildEnsembleArgs) -> Result<()> {
    ensemble::run_build_ensemble(args)
}

pub(crate) fn run_materialize(args: VariantsMaterializeArgs) -> Result<()> {
    materialize::run_materialize(args)
}
