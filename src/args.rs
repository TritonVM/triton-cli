use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use fs_err as fs;
use triton_vm::prelude::BFieldElement;
use triton_vm::prelude::NonDeterminism;
use triton_vm::prelude::Program;
use triton_vm::prelude::PublicInput;
use triton_vm::prelude::VMState;

#[derive(Debug, Clone, Eq, PartialEq, clap::Parser)]
#[command(version, about)]
pub enum Command {
    /// Execute a Triton VM program.
    ///
    /// Run a program to completion, then print the computed result to stdout. Uses
    /// the given input (inline or from a file) and (optional) non-determinism.
    /// If the program does not terminate gracefully, i.e., with instruction `halt`,
    /// the corresponding error is printed to stderr.
    ///
    /// Argument “initial state” conflicts with all of “program”, “input”, “input
    /// file”, and “non-determinism”. Argument “input” conflicts with “input file”.
    Run(RunArgs),

    /// Produce a STARK proof and a corresponding claim, attesting to the correct
    /// execution of a Triton VM program.
    ///
    /// Note that all arithmetic is in the prime field with 2^64 - 2^32 + 1
    /// elements. If the provided public input or secret input contains elements
    /// larger than this, proof generation will be aborted.
    ///
    /// The program executed by Triton VM must terminate gracefully, i.e., with
    /// instruction `halt`. If the program crashes, _e.g._, due to an out-of-bounds
    /// instruction pointer or a failing `assert` instruction, proof generation will
    /// fail.
    ///
    /// Argument “initial state” conflicts with all of “program”, “input”, “input
    /// file”, and “non-determinism”. Argument “input” conflicts with “input file”.
    Prove {
        #[command(flatten)]
        args: RunArgs,

        #[command(flatten)]
        artifacts: ProofArtifacts,
    },

    /// Verify a (Claim, Proof)-pair about the correct execution of a Triton VM
    /// program.
    Verify(ProofArtifacts),
}

/// The arguments required for executing a Triton VM program.
//
// Unfortunately, clap does not support deriving `clap::Args` for enums yet.
// The workaround is to define a struct, declare it as a required group, and
// prohibit the group being mentioned more than once. In effect, this means the
// group has to be named exactly once – it's a worse enum!
//
// A significant downside is that clap cannot communicate which of the
// “variants” was selected. To the best of my knowledge, this has to be done
// by checking for the absence of a field, like `initial_state.is_none()`. 🤦
//
// Relevant issues:
// - <https://github.com/clap-rs/clap/issues/2621>
// - <https://github.com/clap-rs/clap/pull/5700>
#[derive(Debug, Clone, Eq, PartialEq, clap::Args)]
pub struct RunArgs {
    #[arg(
        long,
        conflicts_with = "program",
        conflicts_with = "input",
        conflicts_with = "input_file",
        conflicts_with = "non_determinism",
        value_name = "json file"
    )]
    pub initial_state: Option<String>,

    #[command(flatten)]
    pub separate_files: SeparateFilesRunArgs,
}

#[derive(Debug, Clone, Eq, PartialEq, clap::Args)]
pub struct SeparateFilesRunArgs {
    #[arg(long, value_name = "file")]
    pub program: Option<String>,

    #[command(flatten)]
    pub public_input: Option<InputArgs>,

    #[arg(long)]
    pub non_determinism: Option<String>,
}

// Another “fake enum” – see `RunArgs` for a more detailed explanation.
#[derive(Debug, Clone, Eq, PartialEq, clap::Args)]
pub struct InputArgs {
    #[arg(long, conflicts_with = "input_file")]
    pub input: Option<String>,

    #[arg(long, value_name = "file")]
    pub input_file: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, clap::Args)]
pub struct ProofArtifacts {
    #[arg(long, value_name = "file", default_value_t = String::from("triton.claim"))]
    pub claim: String,

    #[arg(long, value_name = "file", default_value_t = String::from("triton.proof"))]
    pub proof: String,
}

impl RunArgs {
    pub fn parse(self) -> Result<VMState> {
        if let Some(initial_state) = self.initial_state {
            let file = fs::File::open(initial_state)?;
            let reader = std::io::BufReader::new(file);
            return Ok(serde_json::from_reader(reader)?);
        }

        let SeparateFilesRunArgs {
            program,
            public_input,
            non_determinism,
        } = self.separate_files;

        let Some(program) = program else {
            bail!("error: either argument “initial state” or ”program“ must be supplied");
        };
        let code = fs::read_to_string(program)?;

        // own the error to work around lifetime issues
        let program = Program::from_code(&code).map_err(|err| anyhow!("{err}"))?;

        let public_input = public_input
            .map(|args| args.parse())
            .transpose()?
            .unwrap_or_default();
        let non_determinism = if let Some(non_det) = non_determinism {
            let non_det_file = fs::File::open(non_det)?;
            let non_det_reader = std::io::BufReader::new(non_det_file);
            serde_json::from_reader(non_det_reader)?
        } else {
            NonDeterminism::default()
        };

        Ok(VMState::new(program, public_input, non_determinism))
    }
}

impl InputArgs {
    pub fn parse(self) -> Result<PublicInput> {
        let input = if let Some(input_file) = self.input_file {
            fs::read_to_string(input_file)?
        } else if let Some(input) = self.input {
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
}
