use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use fs_err as fs;
use itertools::Itertools;
use triton_vm::prelude::NonDeterminism;
use triton_vm::prelude::PublicInput;

use crate::args::Command;
use crate::args::ProofArtifacts;
use crate::args::RunArgs;

mod args;

fn main() -> Result<ExitCode> {
    human_panic::setup_panic!();

    match Command::parse() {
        Command::Run(args) => run(args),
        Command::Prove { args, artifacts } => prove(args, artifacts),
        Command::Verify(artifacts) => verify(artifacts),
    }
}

fn run(args: RunArgs) -> Result<ExitCode> {
    let mut vm_state = args.parse()?;
    vm_state.run()?;
    if !vm_state.public_output.is_empty() {
        println!("{}", vm_state.public_output.iter().join(", "));
    }

    Ok(ExitCode::SUCCESS)
}

fn prove(args: RunArgs, artifacts: ProofArtifacts) -> Result<ExitCode> {
    let vm_state = args.parse()?;
    let input = PublicInput::new(vm_state.public_input.into());
    let non_determinism = NonDeterminism::new(vm_state.secret_individual_tokens)
        .with_digests(vm_state.secret_digests)
        .with_ram(vm_state.ram);
    let (_, claim, proof) = triton_vm::prove_program(vm_state.program, input, non_determinism)?;

    let claim_file = fs::File::create(artifacts.claim)?;
    serde_json::to_writer(claim_file, &claim)?;

    let proof_file = fs::File::create(artifacts.proof)?;
    bincode::serialize_into(proof_file, &proof)?;

    Ok(ExitCode::SUCCESS)
}

fn verify(artifacts: ProofArtifacts) -> Result<ExitCode> {
    let claim_file = fs::File::open(artifacts.claim)?;
    let claim = serde_json::from_reader(claim_file)?;

    let proof_file = fs::File::open(artifacts.proof)?;
    let proof = bincode::deserialize_from(proof_file)?;

    if triton_vm::verify(triton_vm::stark::Stark::default(), &claim, &proof) {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}
