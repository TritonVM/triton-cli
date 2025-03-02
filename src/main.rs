use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use itertools::Itertools;
use strum::IntoEnumIterator;
use triton_vm::aet::AlgebraicExecutionTrace;
use triton_vm::prelude::Claim;
use triton_vm::prelude::Stark;
use triton_vm::prelude::TableId;
use triton_vm::prelude::VM;

use crate::args::Args;
use crate::args::Command;
use crate::args::Flags;
use crate::args::ProofArtifacts;
use crate::args::RunArgs;

const SUCCESS: ExitCode = ExitCode::SUCCESS;
const FAILURE: ExitCode = ExitCode::FAILURE;

mod args;

fn main() -> Result<ExitCode> {
    human_panic::setup_panic!();

    let Args { flags, command } = Args::parse();
    match command {
        Command::Run(args) => run(flags, args),
        Command::Prove { args, artifacts } => prove(flags, args, artifacts),
        Command::Verify(artifacts) => verify(flags, artifacts),
    }
}

fn run(flags: Flags, args: RunArgs) -> Result<ExitCode> {
    let (program, input, non_determinism) = args.parse()?;

    let output = if flags.profile {
        let (output, profile) =
            VM::profile(program.clone(), input.clone(), non_determinism.clone())?;
        let (aet, _) = VM::trace_execution(program, input, non_determinism)?;
        println!("{profile}\n");
        print_aet(aet);
        output
    } else {
        VM::run(program, input, non_determinism)?
    };
    if !output.is_empty() {
        println!("{}", output.iter().join(", "));
    }

    Ok(SUCCESS)
}

fn prove(flags: Flags, args: RunArgs, artifacts: ProofArtifacts) -> Result<ExitCode> {
    let (program, input, non_determinism) = args.parse()?;

    triton_vm::profiler::start("Triton VM – Prove");
    let claim = Claim::about_program(&program).with_input(input.clone());
    let (aet, public_output) = VM::trace_execution(program, input, non_determinism)?;
    let claim = claim.with_output(public_output);
    let proof = Stark::default().prove(&claim, &aet)?;

    if flags.profile {
        let padded_height = aet.padded_height();
        let profile = triton_vm::profiler::finish()
            .with_cycle_count(aet.processor_trace.nrows())
            .with_padded_height(padded_height)
            .with_fri_domain_len(fri_domain_length(padded_height)?);
        println!("{profile}");
    }

    artifacts.write(&claim, &proof)?;

    Ok(SUCCESS)
}

fn verify(flags: Flags, artifacts: ProofArtifacts) -> Result<ExitCode> {
    let (claim, proof) = artifacts.read()?;

    triton_vm::profiler::start("Triton VM – Verify");
    let verdict = triton_vm::verify(Stark::default(), &claim, &proof);
    if flags.profile {
        let padded_height = proof.padded_height()?;
        let profile = triton_vm::profiler::finish()
            .with_padded_height(padded_height)
            .with_fri_domain_len(fri_domain_length(padded_height)?);
        println!("{profile}");
    }

    let exit_code = if verdict { SUCCESS } else { FAILURE };
    Ok(exit_code)
}

fn fri_domain_length(padded_height: usize) -> Result<usize> {
    let fri = Stark::default().fri(padded_height)?;
    Ok(fri.domain.length)
}

fn print_aet(aet: AlgebraicExecutionTrace) {
    let height_str = "Height";
    let max_height = aet.height().height;
    let height_len = std::cmp::max(max_height.to_string().len(), height_str.len());

    println!("| Table     | {: >height_len$} | Dominates |", height_str);
    println!("|:----------|-{:->height_len$}:|----------:|", "");
    for id in TableId::iter() {
        let height = aet.height_of_table(id);
        let dominates = if height == max_height { "yes" } else { "no" };
        println!("| {id:<9} | {height:>height_len$} | {dominates:>9} |");
    }
    println!();
    println!("Padded height: 2^{}", aet.padded_height().ilog2());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_table_label_len_is_9() {
        let max_table_label_len = TableId::iter()
            .map(|id| id.to_string().len())
            .max()
            .unwrap();
        assert_eq!(9, max_table_label_len);
    }
}
