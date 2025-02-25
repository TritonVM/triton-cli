use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use fs_err as fs;
use triton_vm::prelude::Claim;
use triton_vm::prelude::NonDeterminism;
use triton_vm::prelude::Program;
use triton_vm::prelude::Proof;
use triton_vm::prelude::PublicInput;
use triton_vm::prelude::VMState;

#[derive(Debug, Clone, Eq, PartialEq, clap::Parser)]
#[command(version, about)]
pub struct Args {
    #[command(flatten)]
    pub flags: Flags,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Eq, PartialEq, clap::Subcommand)]
pub enum Command {
    /// Execute a Triton VM program.
    ///
    /// Run a program to completion, then print the computed result to stdout. Uses
    /// the given input (inline or from a file) and (optional) non-determinism.
    /// If the program does not terminate gracefully, the corresponding error is
    /// printed to stderr.
    Run(RunArgs),

    /// Produce a STARK proof and a corresponding claim, attesting to the correct
    /// execution of a Triton VM program.
    ///
    /// Note that all arithmetic is in the prime field with 2^64 - 2^32 + 1
    /// elements. If the provided public input or secret input contains elements
    /// larger than this, proof generation will be aborted.
    ///
    /// The program executed by Triton VM must terminate gracefully. If the program
    /// crashes, _e.g._, due to an out-of-bounds instruction pointer or a failing
    /// `assert` instruction, proof generation will fail.
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

#[derive(Debug, Clone, Eq, PartialEq, clap::Args)]
pub struct Flags {
    /// Print command-dependent profiling information.
    #[arg(long, default_value_t = false)]
    pub profile: bool,
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
    /// The entire initial state, json-encoded. Easiest to obtain programmatically.
    ///
    /// Note that the initial state is not used as is. Instead, the program, public
    /// input, and non-determinism are extracted, then used as if they were passed
    /// separately. Any custom instruction pointer, Sponge state, etc. are
    /// discarded, as generating a valid proof would be impossible otherwise.
    ///
    /// Conflicts with “program”, “input”, “input file”, and “non-determinism”.
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
    /// A file containing a list of Triton instructions.
    #[arg(long, value_name = "file")]
    pub program: Option<String>,

    #[command(flatten)]
    pub public_input: Option<InputArgs>,

    /// A file containing the entire non-determinism, json-encoded.
    #[arg(long)]
    pub non_determinism: Option<String>,
}

// Another “fake enum” – see `RunArgs` for a more detailed explanation.
#[derive(Debug, Clone, Eq, PartialEq, clap::Args)]
pub struct InputArgs {
    /// A comma-separated list of the base field elements the program can use as its
    /// public input.
    ///
    /// Conflicts with “input file”.
    #[arg(long, conflicts_with = "input_file")]
    pub input: Option<String>,

    /// A file containing a comma-separated list of base field elements the program
    /// can use as its input.
    ///
    /// Conflicts with “input”.
    #[arg(long, value_name = "file")]
    pub input_file: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, clap::Args)]
pub struct ProofArtifacts {
    /// The file of the claim that is to be proven or verified.
    #[arg(long, value_name = "file", default_value_t = String::from("triton.claim"))]
    pub claim: String,

    /// The file of the proof for the claim that is to be proven or verified.
    #[arg(long, value_name = "file", default_value_t = String::from("triton.proof"))]
    pub proof: String,
}

impl RunArgs {
    pub fn parse(self) -> Result<(Program, PublicInput, NonDeterminism)> {
        if let Some(initial_state) = self.initial_state {
            let file = fs::File::open(initial_state)?;
            let reader = std::io::BufReader::new(file);
            let state: VMState = serde_json::from_reader(reader)?;
            let input = PublicInput::new(state.public_input.into());
            let non_determinism = NonDeterminism::new(state.secret_individual_tokens)
                .with_digests(state.secret_digests)
                .with_ram(state.ram);
            return Ok((state.program, input, non_determinism));
        }

        let SeparateFilesRunArgs {
            program,
            public_input,
            non_determinism,
        } = self.separate_files;
        let Some(program) = program else {
            bail!("error: either argument “initial state” or ”program“ must be supplied");
        };

        let program = Self::parse_program(program)?;
        let public_input = Self::parse_public_input(public_input)?;
        let non_determinism = Self::parse_non_determinism(non_determinism)?;

        Ok((program, public_input, non_determinism))
    }

    fn parse_program(path: String) -> Result<Program> {
        let code = fs::read_to_string(path)?;

        // own the error to work around lifetime issues
        let program = Program::from_code(&code).map_err(|err| anyhow!("{err}"))?;

        Ok(program)
    }

    fn parse_public_input(public_input: Option<InputArgs>) -> Result<PublicInput> {
        let Some(input_args) = public_input else {
            return Ok(PublicInput::default());
        };

        input_args.parse()
    }

    fn parse_non_determinism(non_determinism: Option<String>) -> Result<NonDeterminism> {
        let Some(path) = non_determinism else {
            return Ok(NonDeterminism::default());
        };
        let file = fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);

        Ok(serde_json::from_reader(reader)?)
    }
}

impl InputArgs {
    pub fn parse(self) -> Result<PublicInput> {
        let input = self
            .input_file
            .map(fs::read_to_string)
            .transpose()?
            .or(self.input)
            .unwrap_or_default()
            .split(',')
            .map(|i| i.trim().parse())
            .collect::<Result<_, _>>()?;

        Ok(PublicInput::new(input))
    }
}

impl ProofArtifacts {
    pub fn read(&self) -> Result<(Claim, Proof)> {
        let claim_file = fs::File::open(&self.claim)?;
        let claim = serde_json::from_reader(claim_file)?;

        let proof_file = fs::File::open(&self.proof)?;
        let proof = bincode::deserialize_from(proof_file)?;

        Ok((claim, proof))
    }

    pub fn write(&self, claim: &Claim, proof: &Proof) -> Result<()> {
        let claim_file = fs::File::create(&self.claim)?;
        serde_json::to_writer(claim_file, claim)?;

        let proof_file = fs::File::create(&self.proof)?;
        bincode::serialize_into(proof_file, proof)?;

        Ok(())
    }
}
