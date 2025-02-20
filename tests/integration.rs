use std::io::Write;
use std::ops::Deref;
use std::ops::DerefMut;

use itertools::Itertools;
use tempfile::NamedTempFile;

/// Wrapper around [`assert_cmd::Command`] and [`tempfile::TempDir`] to keep the
/// temporary directory alive for the duration of the test.
#[derive(Debug)]
struct Command {
    /// Only needed to keep the temporary directory for the `command` in scope.
    // If the TempDir was dropped instead, the temporary directory would be deleted,
    // and command execution would fail. If no temporary directory was used in the
    // first place, tests could interfere with each other and leave files behind.
    #[expect(unused)]
    dir: tempfile::TempDir,

    command: assert_cmd::Command,
}

impl Deref for Command {
    type Target = assert_cmd::Command;

    fn deref(&self) -> &Self::Target {
        &self.command
    }
}

impl DerefMut for Command {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.command
    }
}

fn command() -> Command {
    let dir = tempfile::tempdir().unwrap();
    let command = command_in_dir(&dir);

    Command { dir, command }
}

// Not `Command` but `assert_cmd::Command` because the `TempDir` lives longer
// than this function.
fn command_in_dir(dir: &tempfile::TempDir) -> assert_cmd::Command {
    let mut command = assert_cmd::Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    command.current_dir(dir);

    command
}

/// Create a new named, temporary file with the given contents.
/// The security implications of [`NamedTempFile`] apply.
//
// Although usually, only the path to the file is needed, the file itself is
// returned for lifetime reasons.
fn temp_file(content: impl std::fmt::Display) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{content}").unwrap();

    file
}

#[test]
fn help() {
    command().arg("help").assert().success();
}

#[test]
fn run_without_arguments() {
    command()
        .arg("run")
        .assert()
        .stderr(predicates::str::contains("argument"))
        .failure();
}

#[test]
fn run_trivial_program() {
    let program = temp_file("halt");

    command()
        .args(["run", "--program", program.path().to_str().unwrap()])
        .assert()
        .stdout("")
        .success();
}

#[test]
fn run_program_with_input() {
    let program = temp_file("read_io 1 push 42 eq assert halt");
    let program_path = program.path().to_str().unwrap();

    command()
        .args(["run", "--program", program_path, "--input", "42"])
        .assert()
        .stdout("")
        .success();
}

#[test]
fn run_program_add_with_input_write_output() {
    let program = temp_file("read_io 2 add write_io 1 halt");
    let program_path = program.path().to_str().unwrap();

    command()
        .args(["run", "--program", program_path, "--input", "42,58"])
        .assert()
        .stdout("100\n")
        .success();
}

#[test]
fn run_program_mul_with_input_file_write_output() {
    let program = temp_file("read_io 2 mul write_io 1 halt");
    let program_path = program.path().to_str().unwrap();

    let input = temp_file("-17, 2221");
    let input_path = input.path().to_str().unwrap();

    command()
        .args(["run", "--program", program_path, "--input-file", input_path])
        .assert()
        .stdout("18446744069414546564\n")
        .success();
}

#[test]
fn run_program_with_non_determinism_write_output() {
    let program = temp_file("divine 1 write_io 1 halt");
    let non_det = temp_file(r#"{"individual_tokens":[255],"digests":[],"ram":{}}"#);

    command()
        .arg("run")
        .args(["--program", program.path().to_str().unwrap()])
        .args(["--non-determinism", non_det.path().to_str().unwrap()])
        .assert()
        .stdout("255\n")
        .success();
}

#[test]
fn run_and_prove_initial_state_conflicts_with_other_arguments() {
    let conflicting_args = [
        ["--program", "b.tasm"],
        ["--input", "1,2,3"],
        ["--input-file", "i.txt"],
        ["--non-determinism", "n.json"],
    ];

    for (the_command, conflicting_arg) in ["run", "prove"]
        .into_iter()
        .cartesian_product(conflicting_args)
    {
        command()
            .arg(the_command)
            .args(["--initial-state", "state.json"])
            .args(conflicting_arg)
            .assert()
            .stderr(predicates::str::contains("cannot be used with"))
            .failure();
    }
}

#[test]
fn prove_verify_trivial_program() {
    let program = temp_file("halt");

    let dir = tempfile::tempdir().unwrap();
    command_in_dir(&dir)
        .args(["prove", "--program", program.path().to_str().unwrap()])
        .assert()
        .stdout("")
        .success();
    command_in_dir(&dir)
        .arg("verify")
        .assert()
        .stdout("")
        .success();
}

#[test]
fn prove_verify_trivial_program_to_dedicated_files() {
    let program = temp_file("halt");
    let claim_file = temp_file("");
    let proof_file = temp_file("");

    let claim_args = ["--claim", claim_file.path().to_str().unwrap()];
    let proof_args = ["--proof", proof_file.path().to_str().unwrap()];

    let dir = tempfile::tempdir().unwrap();
    command_in_dir(&dir)
        .arg("prove")
        .args(["--program", program.path().to_str().unwrap()])
        .args(claim_args)
        .args(proof_args)
        .assert()
        .stdout("")
        .success();
    command_in_dir(&dir)
        .arg("verify")
        .args(claim_args)
        .args(proof_args)
        .assert()
        .stdout("")
        .success();
}

#[test]
fn prove_verify_program_with_input() {
    let program = temp_file("read_io 1 push 42 eq assert halt");
    let program_path = program.path().to_str().unwrap();

    let dir = tempfile::tempdir().unwrap();
    command_in_dir(&dir)
        .args(["prove", "--program", program_path, "--input", "42"])
        .assert()
        .stdout("")
        .success();
    command_in_dir(&dir)
        .arg("verify")
        .assert()
        .stdout("")
        .success();
}

#[test]
fn prove_verify_program_with_input_file() {
    let program = temp_file("read_io 4 mul mul mul write_io 1 halt");
    let input = temp_file("17, 19, 23, 29");

    let program_path = program.path().to_str().unwrap();
    let input_path = input.path().to_str().unwrap();

    let dir = tempfile::tempdir().unwrap();
    command_in_dir(&dir)
        .arg("prove")
        .args(["--program", program_path])
        .args(["--input-file", input_path])
        .assert()
        .stdout("")
        .success();
    command_in_dir(&dir)
        .arg("verify")
        .assert()
        .stdout("")
        .success();
}

#[test]
fn prove_verify_program_with_non_determinism() {
    let program = temp_file("push 10 read_mem 3 write_io 3 halt");
    let non_det = temp_file(
        r#"{"individual_tokens":[255],"digests":[],"ram":{"8": 100, "9": 200, "10": 300}}"#,
    );

    let dir = tempfile::tempdir().unwrap();
    command_in_dir(&dir)
        .arg("prove")
        .args(["--program", program.path().to_str().unwrap()])
        .args(["--non-determinism", non_det.path().to_str().unwrap()])
        .assert()
        .stdout("")
        .success();
    command_in_dir(&dir)
        .arg("verify")
        .assert()
        .stdout("")
        .success();
}
