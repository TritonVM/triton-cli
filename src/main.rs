use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use itertools::Itertools;
use triton_vm::prelude::Stark;

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
        let (output, profile) = triton_vm::vm::VM::profile(program, input, non_determinism)?;
        println!("{profile}");
        output
    } else {
        triton_vm::vm::VM::run(program, input, non_determinism)?
    };
    if !output.is_empty() {
        println!("{}", output.iter().join(", "));
    }

    Ok(SUCCESS)
}

fn prove(flags: Flags, args: RunArgs, artifacts: ProofArtifacts) -> Result<ExitCode> {
    let (program, input, non_determinism) = args.parse()?;

    triton_vm::profiler::start("Triton VM – Prove");
    let (_, claim, proof) = triton_vm::prove_program(program, input, non_determinism)?;
    if flags.profile {
        println!("{}", triton_vm::profiler::finish());
    }
    artifacts.write(&claim, &proof)?;

    Ok(SUCCESS)
}

fn verify(flags: Flags, artifacts: ProofArtifacts) -> Result<ExitCode> {
    let (claim, proof) = artifacts.read()?;

    triton_vm::profiler::start("Triton VM – Verify");
    let verdict = triton_vm::verify(Stark::default(), &claim, &proof);
    if flags.profile {
        println!("{}", triton_vm::profiler::finish());
    }

    let exit_code = if verdict { SUCCESS } else { FAILURE };
    Ok(exit_code)
}
