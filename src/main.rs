use std::process::ExitCode;

use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use clap::Parser;
use fs_err as fs;
use itertools::Itertools;
use triton_vm::prelude::BFieldElement;
use triton_vm::prelude::Program;
use triton_vm::prelude::PublicInput;
use triton_vm::prelude::VMState;
use triton_vm::vm::NonDeterminism;

use crate::args::Command;
use crate::args::InputArgs;
use crate::args::ProofArtifacts;
use crate::args::RunArgs;
use crate::args::SeparateFilesRunArgs;

mod args;

fn main() -> Result<ExitCode> {
    match Command::parse() {
        Command::Run(args) => run(args),
        Command::Prove { args, artifacts } => prove(args, artifacts),
        Command::Verify(artifacts) => verify(artifacts),
    }
}

fn run(args: RunArgs) -> Result<ExitCode> {
    let mut vm_state = parse_run_args(args)?;
    vm_state.run()?;
    if !vm_state.public_output.is_empty() {
        println!("{}", vm_state.public_output.iter().join(", "));
    }

    Ok(ExitCode::SUCCESS)
}

fn prove(args: RunArgs, artifacts: ProofArtifacts) -> Result<ExitCode> {
    let VMState {
        program,
        public_input,
        secret_individual_tokens,
        secret_digests,
        ram,
        ..
    } = parse_run_args(args)?;
    let input = PublicInput::new(public_input.into());
    let non_determinism = NonDeterminism::new(secret_individual_tokens)
        .with_digests(secret_digests)
        .with_ram(ram);
    let (_, claim, proof) = triton_vm::prove_program(program, input, non_determinism)?;

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

fn parse_run_args(args: RunArgs) -> Result<VMState> {
    let RunArgs {
        initial_state,
        separate_files,
    } = args;

    let vm_state = if let Some(initial_state) = initial_state {
        let file = fs::File::open(initial_state)?;
        let reader = std::io::BufReader::new(file);
        serde_json::from_reader(reader)?
    } else {
        let SeparateFilesRunArgs {
            program,
            public_input,
            non_determinism,
        } = separate_files;

        let Some(program) = program else {
            bail!("error: either argument “initial state” or ”program“ must be supplied");
        };
        let code = fs::read_to_string(program)?;

        // own the error to work around lifetime issues
        let program = Program::from_code(&code).map_err(|err| anyhow!("({err}"))?;

        let public_input = parse_public_input(public_input)?;
        let non_determinism = if let Some(non_det) = non_determinism {
            let non_det_file = fs::File::open(non_det)?;
            let non_det_reader = std::io::BufReader::new(non_det_file);
            serde_json::from_reader(non_det_reader)?
        } else {
            NonDeterminism::default()
        };

        VMState::new(program, public_input, non_determinism)
    };

    Ok(vm_state)
}

fn parse_public_input(args: Option<InputArgs>) -> Result<PublicInput> {
    let Some(InputArgs { input, input_file }) = args else {
        return Ok(PublicInput::default());
    };

    let input = if let Some(input_file) = input_file {
        fs::read_to_string(input_file)?
    } else if let Some(input) = input {
        input
    } else {
        return Ok(PublicInput::default());
    };

    let input = input
        .split(',')
        .map(|i| i.trim().parse())
        .collect::<Result<Vec<i64>, _>>()?;
    let input = input.into_iter().map(BFieldElement::from).collect();

    Ok(PublicInput::new(input))
}
